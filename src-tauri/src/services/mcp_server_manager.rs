use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{error, info, warn};

use crate::{
    app::error::{AppError, AppResult},
    domain::{
        offline_job::{OfflineJob, OfflineJobStatus},
        runtime_params::RuntimeParams,
        settings::McpSettings,
        voice_separation::{VoicePostProcessConfig, VoiceSeparationJob, VoiceSeparationStem},
    },
    services::{
        asset_cache::AssetCache,
        offline_job_manager::{CreateOfflineAudioJobRequest, CreateOfflineTextJobRequest, OfflineJobManager},
        settings_manager::SettingsManager,
        voice_separation_manager::{CreateVoiceSeparationJobRequest, VoiceSeparationManager},
    },
};

const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Clone, Debug)]
struct McpServerContext {
    settings: Arc<SettingsManager>,
    offline_jobs: Arc<OfflineJobManager>,
    asset_cache: Arc<AssetCache>,
    voice_separation: Arc<VoiceSeparationManager>,
}

#[derive(Debug)]
struct RunningMcpServer {
    settings: McpSettings,
    shutdown: Option<mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

#[derive(Debug)]
pub struct McpServerManager {
    context: McpServerContext,
    running: Mutex<Option<RunningMcpServer>>,
}

impl McpServerManager {
    pub fn new(
        settings: Arc<SettingsManager>,
        offline_jobs: Arc<OfflineJobManager>,
        asset_cache: Arc<AssetCache>,
        voice_separation: Arc<VoiceSeparationManager>,
    ) -> Self {
        Self {
            context: McpServerContext {
                settings,
                offline_jobs,
                asset_cache,
                voice_separation,
            },
            running: Mutex::new(None),
        }
    }

    pub fn apply_settings(&self, settings: &McpSettings) -> AppResult<()> {
        settings.validate().map_err(AppError::invalid_settings)?;
        if !settings.enabled {
            self.stop();
            return Ok(());
        }

        let unchanged = self
            .running
            .lock()
            .expect("MCP server manager lock poisoned")
            .as_ref()
            .map(|running| running.settings == *settings)
            .unwrap_or(false);
        if unchanged {
            return Ok(());
        }

        self.stop();
        self.start(settings.clone())
    }

    fn start(&self, settings: McpSettings) -> AppResult<()> {
        let address = format!("{}:{}", settings.host.trim(), settings.port);
        let listener = TcpListener::bind(&address)
            .map_err(|source| AppError::io("binding MCP Streamable HTTP endpoint", source))?;
        listener
            .set_nonblocking(true)
            .map_err(|source| AppError::io("configuring MCP Streamable HTTP listener", source))?;

        let context = self.context.clone();
        let path = settings.path.clone();
        let endpoint = settings.endpoint_url();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let handle = thread::Builder::new()
            .name("voice-cloner-mcp-http".into())
            .spawn(move || serve_http(listener, shutdown_rx, context, path))
            .map_err(|source| AppError::io("starting MCP Streamable HTTP thread", source))?;

        *self.running.lock().expect("MCP server manager lock poisoned") = Some(RunningMcpServer {
            settings,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        });
        info!(endpoint = %endpoint, "MCP Streamable HTTP server started");
        Ok(())
    }

    pub fn stop(&self) {
        let running = self.running.lock().expect("MCP server manager lock poisoned").take();
        if let Some(mut running) = running {
            if let Some(shutdown) = running.shutdown.take() {
                let _ = shutdown.send(());
            }
            if let Some(handle) = running.handle.take() {
                if let Err(source) = handle.join() {
                    error!(?source, "MCP Streamable HTTP server thread join failed");
                } else {
                    info!("MCP Streamable HTTP server stopped");
                }
            }
        }
    }
}

fn serve_http(listener: TcpListener, shutdown_rx: mpsc::Receiver<()>, context: McpServerContext, path: String) {
    loop {
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                let context = context.clone();
                let path = path.clone();
                let _ = thread::Builder::new()
                    .name("voice-cloner-mcp-request".into())
                    .spawn(move || {
                        if let Err(error) = handle_stream(stream, context, &path) {
                            warn!(error = %error, "MCP HTTP request failed");
                        }
                    });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(40));
            }
            Err(error) => {
                error!(error = %error, "MCP HTTP listener failed");
                break;
            }
        }
    }
}

fn handle_stream(mut stream: TcpStream, context: McpServerContext, path: &str) -> AppResult<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|source| AppError::io("configuring MCP HTTP read timeout", source))?;
    let request = read_http_request(&mut stream)?;
    let response = handle_http_request(context, path, request);
    stream
        .write_all(response.as_bytes())
        .map_err(|source| AppError::io("writing MCP HTTP response", source))
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> AppResult<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let header_end = loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|source| AppError::io("reading MCP HTTP request", source))?;
        if read == 0 {
            return Err(AppError::offline_job("empty MCP HTTP request"));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        if buffer.len() > 64 * 1024 {
            return Err(AppError::offline_job("MCP HTTP headers are too large"));
        }
    };

    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| AppError::offline_job("MCP HTTP request line is missing"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| AppError::offline_job("MCP HTTP method is missing"))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| AppError::offline_job("MCP HTTP path is missing"))?
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end + 4;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let read = stream
            .read(&mut chunk)
            .map_err(|source| AppError::io("reading MCP HTTP body", source))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    Ok(HttpRequest { method, path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn handle_http_request(context: McpServerContext, expected_path: &str, request: HttpRequest) -> String {
    let path = request.path.split('?').next().unwrap_or_default();
    if path != expected_path {
        return http_response(404, "application/json", json!({"error": "not found"}).to_string());
    }

    match request.method.as_str() {
        "GET" => http_response(
            200,
            "application/json",
            json!({
                "name": "voice-cloner",
                "transport": "streamable-http",
                "protocolVersion": PROTOCOL_VERSION,
                "endpoint": "POST JSON-RPC requests to this path"
            })
            .to_string(),
        ),
        "POST" => handle_json_rpc_post(context, request.body),
        "OPTIONS" => http_response(204, "text/plain", String::new()),
        _ => http_response(
            405,
            "application/json",
            json!({"error": "method not allowed"}).to_string(),
        ),
    }
}

fn handle_json_rpc_post(context: McpServerContext, body: Vec<u8>) -> String {
    let payload: Value =
        match serde_json::from_slice(&body) {
            Ok(payload) => payload,
            Err(error) => return http_response(
                400,
                "application/json",
                json!({"jsonrpc": "2.0", "id": Value::Null, "error": {"code": -32700, "message": error.to_string()}})
                    .to_string(),
            ),
        };
    let Some(id) = payload.get("id").cloned() else {
        return http_response(202, "text/plain", String::new());
    };
    let method = payload.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = payload.get("params").cloned().unwrap_or_else(|| json!({}));
    let response = match dispatch_json_rpc(context, method, params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": error.to_string()}})
        }
    };
    http_response(200, "application/json", response.to_string())
}

fn http_response(status: u16, content_type: &str, body: String) -> String {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: http://localhost\r\nAccess-Control-Allow-Headers: content-type, mcp-session-id\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    )
}

fn dispatch_json_rpc(context: McpServerContext, method: &str, params: Value) -> AppResult<Value> {
    match method {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list_result()),
        "tools/call" => call_tool(context, params),
        "resources/list" => list_resources(context),
        "resources/read" => read_resource(context, params),
        "prompts/list" => Ok(prompts_list_result()),
        "prompts/get" => get_prompt(params),
        _ => Err(AppError::unsupported(format!("unsupported MCP method: {method}"))),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {
            "tools": { "listChanged": false },
            "resources": { "subscribe": false, "listChanged": false },
            "prompts": { "listChanged": false }
        },
        "serverInfo": {
            "name": "voice-cloner",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "offline_voice_change",
                "description": "离线变声：将输入文本或音频转换为指定音色的音频文件。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "文本输入；与 inputPath 二选一。" },
                        "inputPath": { "type": "string", "description": "本地音频输入路径；与 text 二选一。" },
                        "voiceName": { "type": "string", "description": "目标 FunSpeech 音色名称。" },
                        "runtimeParams": { "type": "object", "description": "变声运行参数，写入 values 对象。" },
                        "outputFormat": { "type": "string", "enum": ["wav"], "default": "wav" }
                    },
                    "required": ["voiceName"]
                }
            },
            {
                "name": "separate_voice",
                "description": "人声分离：从音频或视频源中分离 vocals/noVocals/drums/bass/other。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "sourcePath": { "type": "string", "description": "本地音频或视频源文件路径。" },
                        "model": { "type": "string", "enum": ["htDemucs", "htDemucsFt"], "default": "htDemucs" },
                        "postProcessConfig": { "type": "object", "description": "人声后处理参数。" }
                    },
                    "required": ["sourcePath"]
                }
            }
        ]
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

fn call_tool(context: McpServerContext, params: Value) -> AppResult<Value> {
    let params: ToolCallParams =
        serde_json::from_value(params).map_err(|source| AppError::json("parsing MCP tool call params", source))?;
    match params.name.as_str() {
        "offline_voice_change" => call_offline_voice_change(context, params.arguments),
        "separate_voice" => call_separate_voice(context, params.arguments),
        _ => Err(AppError::unsupported(format!("unknown MCP tool: {}", params.name))),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OfflineVoiceChangeArgs {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    input_path: Option<String>,
    voice_name: String,
    #[serde(default)]
    runtime_params: RuntimeParams,
    #[serde(default = "default_wav")]
    output_format: String,
}

fn default_wav() -> String {
    "wav".into()
}

fn call_offline_voice_change(context: McpServerContext, args: Value) -> AppResult<Value> {
    let args: OfflineVoiceChangeArgs = serde_json::from_value(args)
        .map_err(|source| AppError::json("parsing offline_voice_change arguments", source))?;
    let settings = context.settings.load_or_default()?;
    let created = match (args.text, args.input_path) {
        (Some(text), None) => context.offline_jobs.create_text_job(
            CreateOfflineTextJobRequest {
                text,
                voice_name: args.voice_name,
                runtime_params: args.runtime_params,
                output_format: Some(args.output_format),
            },
            &settings,
        )?,
        (None, Some(input_path)) => context.offline_jobs.create_audio_job(
            CreateOfflineAudioJobRequest {
                input_ref: Some(input_path),
                file_name: None,
                input_bytes: None,
                voice_name: args.voice_name,
                runtime_params: args.runtime_params,
                output_format: Some(args.output_format),
            },
            &settings,
            &context.asset_cache,
        )?,
        _ => {
            return Err(AppError::offline_job(
                "offline_voice_change requires exactly one of text or inputPath",
            ))
        }
    };
    let completed = context
        .offline_jobs
        .start_job(&created.job_id, &settings, &context.asset_cache)?;
    Ok(tool_text_result(offline_job_payload(&completed)))
}

fn offline_job_payload(job: &OfflineJob) -> Value {
    json!({
        "jobId": job.job_id,
        "status": job.status,
        "stage": job.stage,
        "progress": job.progress,
        "localArtifactPath": job.local_artifact_path,
        "resourceUri": format!("voice-cloner://offline-jobs/{}/artifact", job.job_id),
        "errorSummary": job.error_summary
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeparateVoiceArgs {
    source_path: String,
    #[serde(default)]
    model: Option<crate::domain::voice_separation::VoiceSeparationModel>,
    #[serde(default)]
    post_process_config: Option<VoicePostProcessConfig>,
}

fn call_separate_voice(context: McpServerContext, args: Value) -> AppResult<Value> {
    let args: SeparateVoiceArgs =
        serde_json::from_value(args).map_err(|source| AppError::json("parsing separate_voice arguments", source))?;
    let created = context.voice_separation.create_job(CreateVoiceSeparationJobRequest {
        source_path: args.source_path,
        model: args.model,
        post_process_config: args.post_process_config.clone(),
    })?;
    let completed = context
        .voice_separation
        .start_job(&created.job_id, args.post_process_config)?;
    Ok(tool_text_result(voice_separation_payload(&completed)))
}

fn voice_separation_payload(job: &VoiceSeparationJob) -> Value {
    json!({
        "jobId": job.job_id,
        "status": job.status,
        "progress": job.progress,
        "currentStageMessage": job.current_stage_message,
        "resourceUris": {
            "vocals": format!("voice-cloner://voice-separation/{}/stems/vocals", job.job_id),
            "noVocals": format!("voice-cloner://voice-separation/{}/stems/noVocals", job.job_id),
            "drums": format!("voice-cloner://voice-separation/{}/stems/drums", job.job_id),
            "bass": format!("voice-cloner://voice-separation/{}/stems/bass", job.job_id),
            "other": format!("voice-cloner://voice-separation/{}/stems/other", job.job_id)
        },
        "postProcessedVocalsPath": job.post_processed_vocals_path,
        "errorMessage": job.error_message
    })
}

fn tool_text_result(payload: Value) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string())
            }
        ],
        "structuredContent": payload
    })
}

fn list_resources(context: McpServerContext) -> AppResult<Value> {
    let mut resources = Vec::new();
    for job in context.offline_jobs.list_jobs() {
        if job.status == OfflineJobStatus::Completed && job.local_artifact_path.is_some() {
            resources.push(json!({
                "uri": format!("voice-cloner://offline-jobs/{}/artifact", job.job_id),
                "name": format!("Offline voice artifact {}", job.job_id),
                "description": "受控本地离线变声音频路径",
                "mimeType": "text/plain"
            }));
        }
    }

    for job in context.voice_separation.list_jobs() {
        if let Some(stems) = &job.stems {
            for (stem, path) in [
                ("vocals", stems.vocals.as_ref()),
                ("noVocals", stems.no_vocals.as_ref()),
                ("drums", stems.drums.as_ref()),
                ("bass", stems.bass.as_ref()),
                ("other", stems.other.as_ref()),
            ] {
                if path.is_some() {
                    resources.push(json!({
                        "uri": format!("voice-cloner://voice-separation/{}/stems/{stem}", job.job_id),
                        "name": format!("Voice separation {stem} {}", job.job_id),
                        "description": "受控本地人声分离音频路径",
                        "mimeType": "text/plain"
                    }));
                }
            }
        }
    }

    Ok(json!({ "resources": resources }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceReadParams {
    uri: String,
}

fn read_resource(context: McpServerContext, params: Value) -> AppResult<Value> {
    let params: ResourceReadParams =
        serde_json::from_value(params).map_err(|source| AppError::json("parsing MCP resource params", source))?;
    let path = controlled_resource_path(&context, &params.uri)?;
    Ok(json!({
        "contents": [
            {
                "uri": params.uri,
                "mimeType": "text/plain",
                "text": path.to_string_lossy()
            }
        ]
    }))
}

fn controlled_resource_path(context: &McpServerContext, uri: &str) -> AppResult<PathBuf> {
    if let Some(rest) = uri.strip_prefix("voice-cloner://offline-jobs/") {
        let Some(job_id) = rest.strip_suffix("/artifact") else {
            return Err(AppError::offline_job("unsupported offline resource URI"));
        };
        let job = context.offline_jobs.get_job(job_id)?;
        if job.status != OfflineJobStatus::Completed {
            return Err(AppError::offline_job("offline resource is not completed"));
        }
        let expected = context
            .asset_cache
            .offline_artifact_path(job_id, &job.output_format)?
            .path;
        let stored = job
            .local_artifact_path
            .as_ref()
            .map(PathBuf::from)
            .ok_or_else(|| AppError::offline_job("offline resource has no local artifact"))?;
        if stored != expected {
            return Err(AppError::offline_job(
                "offline resource is outside the controlled artifact cache",
            ));
        }
        ensure_existing_resource(expected)
    } else if let Some(rest) = uri.strip_prefix("voice-cloner://voice-separation/") {
        let mut parts = rest.split('/');
        let job_id = parts
            .next()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::offline_job("voice separation job id is required"))?;
        let segment = parts.next();
        let stem = parts.next();
        if segment != Some("stems") || parts.next().is_some() {
            return Err(AppError::offline_job("unsupported voice separation resource URI"));
        }
        let stem = parse_stem(stem.ok_or_else(|| AppError::offline_job("voice separation stem is required"))?)?;
        ensure_existing_resource(context.voice_separation.stem_path(job_id, &stem)?)
    } else {
        Err(AppError::offline_job("unsupported voice-cloner resource URI"))
    }
}

fn parse_stem(value: &str) -> AppResult<VoiceSeparationStem> {
    match value {
        "vocals" => Ok(VoiceSeparationStem::Vocals),
        "noVocals" => Ok(VoiceSeparationStem::NoVocals),
        "drums" => Ok(VoiceSeparationStem::Drums),
        "bass" => Ok(VoiceSeparationStem::Bass),
        "other" => Ok(VoiceSeparationStem::Other),
        _ => Err(AppError::offline_job("unsupported voice separation stem")),
    }
}

fn ensure_existing_resource(path: PathBuf) -> AppResult<PathBuf> {
    if path.exists() && path.is_file() {
        Ok(path)
    } else {
        Err(AppError::offline_job("controlled resource path does not exist"))
    }
}

fn prompts_list_result() -> Value {
    json!({
        "prompts": [
            {
                "name": "explain_offline_voice_change",
                "description": "解释离线变声工具的用途、参数和资源读取方式。",
                "arguments": []
            },
            {
                "name": "explain_voice_separation",
                "description": "解释人声分离工具的用途、参数和资源读取方式。",
                "arguments": []
            }
        ]
    })
}

#[derive(Debug, Deserialize)]
struct PromptGetParams {
    name: String,
}

fn get_prompt(params: Value) -> AppResult<Value> {
    let params: PromptGetParams =
        serde_json::from_value(params).map_err(|source| AppError::json("parsing MCP prompt params", source))?;
    let text = match params.name.as_str() {
        "explain_offline_voice_change" => {
            "离线变声工具 offline_voice_change 用于把文本或本地音频转换为指定音色的 WAV。参数：text 与 inputPath 二选一；voiceName 是目标音色；runtimeParams.values 传递语速、情绪等后端参数；outputFormat 当前固定为 wav。工具返回 jobId、状态、localArtifactPath 和 voice-cloner://offline-jobs/{jobId}/artifact 资源 URI。读取该 resource 只会返回应用缓存中的受控本地音频路径。"
        }
        "explain_voice_separation" => {
            "人声分离工具 separate_voice 用于从本地音频或视频中分离 vocals、noVocals、drums、bass、other。参数：sourcePath 是源文件路径；model 可选 htDemucs 或 htDemucsFt；postProcessConfig 控制降噪、采样率、声道和响度归一化。工具返回 jobId、状态和每个 stem 的 voice-cloner://voice-separation/{jobId}/stems/{stem} 资源 URI。读取 resource 只会返回该任务产出的受控本地音频路径。"
        }
        _ => return Err(AppError::unsupported(format!("unknown MCP prompt: {}", params.name))),
    };
    Ok(json!({
        "description": params.name,
        "messages": [
            {
                "role": "user",
                "content": {
                    "type": "text",
                    "text": text
                }
            }
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::parse_stem;
    use crate::domain::voice_separation::VoiceSeparationStem;

    #[test]
    fn parse_stem_accepts_controlled_values_only() {
        assert_eq!(parse_stem("vocals").unwrap(), VoiceSeparationStem::Vocals);
        assert!(parse_stem("../secret").is_err());
    }
}
