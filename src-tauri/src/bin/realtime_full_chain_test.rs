use std::{fs, path::PathBuf};

use voice_cloner::{
    app::error::{AppError, AppResult},
    build_app_state,
    domain::runtime_params::RuntimeParams,
    services::realtime_full_chain_tester::{run_realtime_full_chain_test, RealtimeFullChainTestRequest},
};

#[tokio::main]
async fn main() -> AppResult<()> {
    let args = CliArgs::parse(std::env::args().skip(1))?;
    let audio_bytes =
        fs::read(&args.audio_path).map_err(|source| AppError::io("reading realtime test audio", source))?;
    let file_name = args
        .audio_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("realtime-test.wav")
        .to_string();

    let state = build_app_state()?;
    let report = run_realtime_full_chain_test(
        &state,
        RealtimeFullChainTestRequest {
            voice_name: args.voice_name,
            file_name,
            audio_bytes,
            runtime_params: RuntimeParams::default(),
            backend_base_url: args.backend_base_url,
            start_monitor: Some(args.start_monitor),
            poll_interval_ms: Some(args.poll_interval_ms),
            drain_grace_ms: Some(args.drain_grace_ms),
            max_duration_ms: Some(args.max_duration_ms),
        },
    )
    .await?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| AppError::realtime_session(format!("serializing full-chain report failed: {error}")))?;
    if let Some(output_path) = args.output_path {
        fs::write(&output_path, json).map_err(|source| AppError::io("writing realtime full-chain report", source))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    audio_path: PathBuf,
    voice_name: String,
    backend_base_url: Option<String>,
    output_path: Option<PathBuf>,
    start_monitor: bool,
    poll_interval_ms: u64,
    drain_grace_ms: u64,
    max_duration_ms: u64,
}

impl CliArgs {
    fn parse(args: impl Iterator<Item = String>) -> AppResult<Self> {
        let mut audio_path = None;
        let mut voice_name = None;
        let mut backend_base_url = None;
        let mut output_path = None;
        let mut start_monitor = true;
        let mut poll_interval_ms = 500;
        let mut drain_grace_ms = 6_000;
        let mut max_duration_ms = 180_000;
        let mut iter = args.peekable();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--audio" => audio_path = Some(required_value(&mut iter, "--audio")?.into()),
                "--voice" => voice_name = Some(required_value(&mut iter, "--voice")?),
                "--server" | "--backend" => backend_base_url = Some(required_value(&mut iter, arg.as_str())?),
                "--output" => output_path = Some(required_value(&mut iter, "--output")?.into()),
                "--no-monitor" => start_monitor = false,
                "--poll-ms" => poll_interval_ms = parse_u64(&mut iter, "--poll-ms")?,
                "--drain-ms" => drain_grace_ms = parse_u64(&mut iter, "--drain-ms")?,
                "--max-ms" => max_duration_ms = parse_u64(&mut iter, "--max-ms")?,
                "--help" | "-h" => return Err(AppError::realtime_session(usage())),
                other => {
                    return Err(AppError::realtime_session(format!(
                        "unknown argument: {other}\n{}",
                        usage()
                    )))
                }
            }
        }

        Ok(Self {
            audio_path: audio_path
                .ok_or_else(|| AppError::realtime_session(format!("--audio is required\n{}", usage())))?,
            voice_name: voice_name
                .ok_or_else(|| AppError::realtime_session(format!("--voice is required\n{}", usage())))?,
            backend_base_url,
            output_path,
            start_monitor,
            poll_interval_ms,
            drain_grace_ms,
            max_duration_ms,
        })
    }
}

fn required_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<String> {
    iter.next()
        .ok_or_else(|| AppError::realtime_session(format!("{flag} requires a value")))
}

fn parse_u64(iter: &mut impl Iterator<Item = String>, flag: &str) -> AppResult<u64> {
    required_value(iter, flag)?
        .parse::<u64>()
        .map_err(|error| AppError::realtime_session(format!("{flag} must be a positive integer: {error}")))
}

fn usage() -> String {
    "usage: cargo run --bin realtime_full_chain_test -- --server 10.0.0.96:8000 --voice 中文女 --audio \"C:\\path\\test.wav\" [--output report.json] [--no-monitor] [--max-ms 180000]".into()
}
