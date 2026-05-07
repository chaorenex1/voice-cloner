# AI Voice Cloner 技术架构设计

## 1. 文档目标

本文定义 Voice Cloner MVP 的技术架构，覆盖：

- 多平台桌面端应用架构
- 本地音频链路与虚拟麦克风输出
- 云端实时推理、离线处理与音色设计服务
- ASR / LLM / TTS / 实时变声后端的可插拔抽象
- 本地配置、自定义音色库与导出链路

本文不覆盖：

- 用户上传样本训练专属声音
- 社区、账号、商业化系统
- 多语言翻译配音
- 移动端架构

## 2. 架构目标

### 2.1 业务目标

- 支持实时变声、离线变声、音色设计三条核心能力
- 让用户在本地桌面端完成设备配置、参数调整、试听、导出
- 将重计算推理能力放在云端，降低本地硬件门槛
- 支持多后端切换：LLM、ASR、TTS、实时变声后端均可配置

### 2.2 技术目标

- 多平台桌面端优先支持 Windows 与 macOS
- 实时链路端到端延迟目标 `<= 150ms`
- 云端后端可横向扩展
- 关键后端抽象解耦，避免与单一供应商强耦合
- 本地与云端职责边界清晰，保证后续可替换性

## 3. 架构原则

### 3.1 本地负责交互与音频控制

本地应用负责：

- 音频设备接入
- 音频采集与播放
- 监听控制
- 虚拟麦克风输出
- 参数调节与 UI 交互
- 本地配置与缓存

本地不负责：

- 重型语音推理
- 音色设计中的 LLM 生成
- 高质量离线合成

### 3.2 云端负责推理与媒体编排

云端负责：

- 实时变声推理会话
- 离线音频/文本转换任务
- 音色设计流水线
- 参考音频与参考文本生成
- 各类模型后端适配

### 3.3 后端统一抽象，产品能力前置

产品层不直接绑定具体供应商，而是通过统一接口访问：

- `ASRProvider`
- `LLMProvider`
- `SpeechSynthesisProvider`
- `RealtimeVoiceProvider`

这样可以让 MVP 先接一个默认后端，后续再扩展可选后端。

## 4. 推荐技术栈

### 4.1 桌面端

- 桌面壳：`Tauri 2`
- 前端 UI：`React + TypeScript`
- 本地原生核心：`Rust`
- 本地配置：`SQLite 或文件型 KV 存储`
- 本地音频缓存：文件系统目录

### 4.2 云端

- 统一后端：`FunSpeech`
- 服务框架：`Python FastAPI` 或等价 HTTP / WebSocket 服务框架
- 模型运行时：ASR / TTS / Realtime Voice / VoxCPM
- 本地文件存储：用于参考音频、离线导出文件、调试工件
- 可观测性：日志、指标、链路追踪

### 4.3 推理后端抽象

- 语音识别：ASR backend
- 音色指令生成：LLM backend
- 语音合成：TTS backend
- 实时变声：`FunSpeech Realtime Voice`

### 4.4 当前默认落地绑定

结合当前项目约束，MVP 默认采用以下绑定：

- ASR 云端项目：`FunSpeech`
- TTS 云端项目：`FunSpeech`
- LLM 服务：桌面端直接调用本地模型服务
- 实时变声 backend：`FunSpeech`

说明：

- `FunSpeech` 当前已经具备较完整的 ASR / TTS / WebSocket 能力，适合作为音色设计与离线文本语音生成的主要云端基础设施
- LLM 直接由桌面端调用本地模型服务，这样音色设计里的提示词编排与迭代能留在本地完成
- 自定义音色注册表放在桌面端本地，但音色管理要与 `FunSpeech` 的 `voice_manager` 同步
- 首次启动从 `FunSpeech voice_manager` 做全量同步，后续新增 / 修改 / 删除走增量同步
- 实时变声、ASR、TTS、Voice Design 都统一收口到 `FunSpeech`

## 5. 总体架构

```text
+--------------------------------------------------------------+
|                      Desktop App (Tauri)                     |
|                                                              |
|  +-------------------+    +-------------------------------+  |
|  | React UI          |    | Rust Native Core             |  |
|  | - 实时变声台      |    | - Device Manager             |  |
|  | - 离线处理页      |    | - Audio Capture Engine       |  |
|  | - 音色设计页      |    | - Playback / Monitor Engine  |  |
|  | - 设置页          |    | - Virtual Mic Adapter        |  |
|  | - 本地音色库      |    | - Realtime Stream Client     |  |
|  +-------------------+    | - Offline Job Client         |  |
|                           | - Local Config Store         |  |
|                           +-------------------------------+  |
+-------------------------------|------------------------------+
                                |
                                | HTTPS / WebSocket
                                v
+--------------------------------------------------------------+
|                   Cloud Services (MVP Reality)               |
|                                                              |
|  +------------------------------------------------------+   |
|  | FunSpeech                                            |   |
|  | - REST ASR                                           |   |
|  | - WebSocket ASR                                      |   |
|  | - REST TTS / OpenAI TTS                              |   |
|  | - WebSocket TTS                                      |   |
|  | - Realtime Voice                                     |   |
|  | - Voice Design (VoxCPM)                              |   |
|  | - voice_manager sync (full sync / incremental sync)  |   |
|  | - Local file storage / model runtime                 |   |
|  +------------------------------------------------------+   |
+--------------------------------------------------------------+
```

补充说明：

- 音色设计链路中的 LLM 不走云端 Worker，而是由桌面端直接调用本地模型服务
- `FunSpeech` 在当前架构中统一承担 ASR、TTS、Realtime Voice、Voice Design 与 `voice_manager`

## 6. 本地应用架构

## 6.1 分层

### UI 层

职责：

- 展示页面与状态
- 管理音色切换、参数调整、任务列表
- 展示连接状态、导出状态、错误提示

不负责：

- 直接操作音频设备
- 直接执行推理

### 应用服务层

职责：

- 封装命令调用与事件订阅
- 管理实时会话生命周期
- 管理离线任务生命周期
- 管理音色设计流程状态

### 原生音频层

职责：

- 麦克风采集
- 音频缓冲
- 本地监听播放
- 虚拟麦克风输出
- 音量检测与基础预处理

### 本地存储层

职责：

- 保存设备设置
- 保存上次角色设置
- 保存最近导出记录
- 保存自定义音色元数据
- 缓存参考音频与预览工件

## 6.2 本地模块划分

### `ui-shell`

管理页面路由与布局：

- 实时变声台
- 离线变声页
- 音色设计页
- 设置页

### `session-manager`

管理实时变声会话：

- 创建会话
- 连接 `FunSpeech` 实时音频流
- 管理开始/停止
- 管理重连
- 管理错误回退

### `audio-engine`

本地音频核心：

- 采集麦克风 PCM
- 监听输出
- 音量电平检测
- 缓冲与抖动控制
- 输出到虚拟麦克风

### `offline-job-manager`

离线任务管理：

- 创建音频转换任务
- 创建文本转语音任务
- 查询任务状态
- 下载导出文件

### `voice-design-manager`

音色设计状态机：

- 录音或文本输入
- 发送 FunSpeech ASR / TTS 请求
- 发送本地 LLM 服务请求
- 展示识别结果与生成结果
- 保存到自定义音色库

### `voice-sync-manager`

管理桌面端与 `FunSpeech voice_manager` 的同步：

- 首次启动全量同步
- 新增音色增量同步
- 修改音色增量同步
- 删除音色增量同步
- 同步失败重试与冲突提示

### `settings-manager`

管理本地配置：

- 输入/输出设备
- LLM backend 配置
- TTS backend 配置
- ASR backend 配置
- FunSpeech realtime 配置

### `asset-cache`

管理本地媒体缓存：

- 音色试听缓存
- 参考音频缓存
- 离线导出下载缓存

## 7. 云端架构

这一节只描述当前已经明确的 MVP 云端现实形态，不再使用泛化的“控制平面 / 微服务拆分”说法。

## 7.1 云端边界

MVP 云端实际就是一个统一语音后端：`FunSpeech`

### `FunSpeech`

负责：

- REST ASR
- WebSocket ASR
- REST TTS
- OpenAI 兼容 TTS
- WebSocket TTS
- Realtime Voice
- 音色设计接口
- `voice_manager`
- 首次启动全量同步所需的音色读取
- 后续新增 / 修改 / 删除的增量同步

不负责：

- 桌面端本地 LLM 调用
- 统一音色注册表权威
- 桌面端 UI 与本地配置

## 7.2 FunSpeech 内部职责

从 `voice-cloner` 的视角看，`FunSpeech` 需要暴露这些产品能力：

### `REST ASR`

用途：

- 离线音频识别
- 音色设计中的语音描述识别

接口面：

- `POST /stream/v1/asr`

### `WebSocket ASR`

用途：

- 桌面端连续语音输入
- 更低交互延迟的音色设计描述输入

接口面：

- `WS /ws/v1/asr`

### `REST / WebSocket TTS`

用途：

- 离线文本转语音
- 基于唯一音色名称生成目标角色语音

接口面：

- `POST /stream/v1/tts`
- `POST /openai/v1/audio/speech`
- `WS /ws/v1/tts`

### `Realtime Voice`

用途：

- 实时变声主链路
- 运行中参数更新
- 运行中角色切换

接口面：

- `WS /ws/v1/realtime/voice`

### `Voice Design`

用途：

- 接收桌面端本地 LLM 生成的音色设计指令
- 使用 `VoxCPM` 生成参考音频

接口面：

- `POST /voices/v1/voice-design`

### `Voice Asset Runtime Loader`

用途：

- 首次启动从 `voice_manager` 全量同步到桌面端
- 后续新增 / 修改 / 删除走增量同步
- 按唯一 `voice_name` 保持桌面端注册表与 `voice_manager` 一致

接口面：

- `GET /voices/v1/sync`
- `POST /voices/v1/register`
- `POST /voices/v1/update`
- `POST /voices/v1/delete`
- `POST /voices/v1/refresh`

## 9. 核心数据模型

## 9.1 `VoicePreset`

系统预置角色：

- `name`
- `description`
- `preview_audio_url`
- `reference_text`
- `default_params`
- `backend_binding`

## 9.2 `CustomVoiceProfile`

自定义音色条目：

- `voice_name`（本地唯一）
- `source_prompt_text`
- `asr_text`
- `voice_instruction`
- `reference_audio_path`
- `reference_text`
- `sync_status`
- `last_synced_at`
- `created_at`

## 9.3 `RealtimeSession`

- `session_id`
- `voice_name`
- `runtime_params`
- `backend_name`
- `status`
- `created_at`

## 9.4 `OfflineJob`

- `job_id`
- `input_type` (`audio` | `text`)
- `input_ref`
- `voice_name`
- `runtime_params`
- `output_format`
- `status`
- `artifact_url`

## 9.5 `AppSettings`

- `input_device_id`
- `output_device_id`
- `virtual_mic_enabled`
- `llm_backend_config`
- `tts_backend_config`
- `asr_backend_config`
- `funspeech_realtime_config`

## 10. 核心流程设计

## 10.1 实时变声流程

```text
麦克风采集
-> 本地缓冲 / 音量检测
-> WebSocket 发送到 FunSpeech Realtime Voice
-> FunSpeech 执行实时变声推理
-> 返回连续音频块
-> 本地监听播放
-> 输出到虚拟麦克风
```

### 关键点

- 桌面端始终掌握开始/停止权
- 云端会话仅负责实时推理，不负责设备控制
- 本地侧要保留最小缓冲区与抖动恢复能力
- 断线后应优先停流并提示，而不是输出脏音频

## 10.2 离线音频变声流程

```text
上传本地音频
-> 调用 FunSpeech
-> FunSpeech 执行音频处理
-> 导出结果写入 FunSpeech 本地文件存储
-> 桌面端轮询或等待结果
-> 下载导出文件
```

### 关键点

- 音频输入与文本输入共用离线任务框架
- 离线任务必须是可恢复、可重试、可查询的
- 导出文件应有统一工件管理
- 若产品要求“保留原音频节奏、停顿、情绪”的真音频转音频效果，则需要独立 Voice Conversion backend；仅靠 `FunSpeech` 的 ASR + TTS 只能实现重建式输出

## 10.3 离线文本转语音流程

```text
输入文本
-> 选择目标角色与参数
-> 调用 FunSpeech TTS
-> 产出音频文件
-> 导出 WAV / MP3
```

### 关键点

- 文本输入产品上属于离线变声页，但技术上是角色语音生成
- 因为与音频变声共享角色、参数、导出链路，所以仍可放在统一任务系统内

## 10.4 音色设计流程

```text
语音或文本描述
-> ASR Worker（若输入为语音）
-> 桌面端调用本地 LLM 服务生成音色设计指令与参考文本
-> FunSpeech Voice Design Service / VoxCPM 生成参考音频
-> 用户试听确认
-> 保存 CustomVoiceProfile 到桌面端本地音色注册表
```

### 关键点

- 这不是训练型克隆，而是“指令驱动的音色设计”
- 桌面端本地音色注册表是自定义音色的权威来源
- FunSpeech 通过唯一音色名称与桌面端建立关联，不维护统一音色注册表 API
- 需要保留识别文本、LLM 指令、参考音频三类中间产物
- 设计失败必须告诉用户失败在哪一层

## 11. 实时链路延迟预算

| 环节 | 目标延迟 |
| --- | --- |
| 本地采集与缓冲 | 10-20ms |
| 本地预处理与封包 | 5-10ms |
| 上行网络 | 20-40ms |
| 云端推理 | 40-60ms |
| 下行网络 | 20-40ms |
| 本地解包与输出 | 10-20ms |
| 合计 | 105-190ms |

### 目标解读

- `理想目标`：压到 `100-150ms`
- `可接受上限`：`<= 150ms`
- 若长期超过 `150ms`，需要优先优化：
  - 区域部署
  - 音频块大小
  - 云端模型推理时间
  - 本地缓冲策略

## 12. 配置系统设计

## 12.1 本地配置结构

建议配置分为三层：

- `device settings`：输入、输出、监听、虚拟麦克风
- `backend settings`：LLM、ASR、TTS、Realtime backend
- `runtime settings`：默认角色、默认参数、导出格式

## 12.2 backend 配置建议

每类 backend 建议包含：

- `provider_name`
- `base_url`
- `api_key_ref`
- `model`
- `timeout_ms`
- `region`
- `extra_options`

### 说明

- `api_key_ref` 不建议明文写入普通配置文件
- 桌面端应通过系统密钥存储或加密机制保存敏感信息

## 13. 本地数据与文件布局

```text
~/voice-cloner/
  settings/
    app-settings.json
  cache/
    preset-preview/
    voice-design-artifacts/
    offline-exports/
  library/
    custom-voices/
      <voice-name>.json
      <voice-name>.wav
```

## 14. 错误处理与恢复

## 14.1 实时变声

- 麦克风不可用：阻止启动并提示重新选择设备
- 云端连接失败：阻止进入运行态并提示网络或 backend 异常
- 推理流中断：立即停流、清空缓冲、提示重试

## 14.2 离线变声

- 文件格式非法：本地拦截
- 文本为空：本地拦截
- 任务失败：保留失败状态与错误摘要
- 下载失败：允许重新下载工件

## 14.3 音色设计

- ASR 失败：允许重录或切换为文本输入
- LLM 失败：允许重新生成指令
- TTS 失败：允许保留指令后再次生成参考音频

## 15. 平台差异与风险

## 15.1 虚拟麦克风

虚拟麦克风是 MVP 成败关键，但也是平台差异最大的部分：

- Windows：通常需要虚拟音频设备或驱动方案
- macOS：通常需要额外系统级音频路由方案
- Linux：MVP 暂不优先实现

因此架构上必须把虚拟麦克风能力放在独立适配层，而不是写死在 UI 或会话逻辑里。

## 15.2 网络依赖

因为本方案采用云端推理：

- 网络质量会直接影响实时体验
- 设置页必须暴露 backend 状态与连接质量
- 需要就近区域部署

## 15.3 供应商耦合

如果把音色设计、离线变声、实时变声全部绑定到一个供应商，会导致：

- 成本不可控
- 质量调优空间变小
- 单点故障放大

因此推荐保持后端抽象层。

## 16. 可观测性设计

## 16.1 本地埋点

- 会话开始/结束
- 实时链路延迟
- 角色切换次数
- 音频设备错误
- 导出成功/失败

## 16.2 云端指标

- WebSocket 会话数
- 实时推理平均时延
- 任务队列长度
- 各 backend 错误率
- 音色设计各阶段失败率

## 16.3 日志关联

建议所有实时会话、离线任务、音色设计任务都带统一 `trace_id`：

- 本地日志
- API 日志
- Worker 日志
- 工件生成日志

## 17. MVP 实施顺序

### Phase 1：本地壳与设置

- 完成 Tauri 桌面壳
- 完成输入/输出设备配置
- 完成本地配置保存

### Phase 2：实时变声主链路

- 完成实时会话创建
- 完成本地采集、监听、输出
- 完成云端实时音频流推理
- 完成虚拟麦克风适配

### Phase 3：离线变声

- 完成音频输入任务
- 完成文本输入任务
- 完成导出链路

### Phase 4：音色设计

- 完成 ASR -> 本地 LLM -> TTS 编排
- 完成自定义音色库存储
- 完成参考音频试听与确认

## 18. 当前推荐决策

### 推荐 1：桌面端选 Tauri + Rust，而不是先做纯 Electron

原因：

- 多平台桌面壳成熟
- Rust 更适合承接音频与原生适配
- 更容易把音频链路与 UI 层分离

### 推荐 2：实时链路优先走 WebSocket

原因：

- MVP 更容易调试
- 与桌面端实现和云端编排更直接
- 适合先把会话与后端抽象打通

### 推荐 3：离线任务统一走异步任务系统

原因：

- 音频输入和文本输入都可复用
- 更容易重试、观测、下载导出结果

### 推荐 4：音色设计明确定位为“指令式设计”，不要假装是训练式克隆

原因：

- 当前 scope 没有训练流程
- 指令式设计更符合 MVP 边界
- 能避免把技术复杂度提前引入

## 19. 外部参考

- Tauri 官方文档：`https://tauri.app/start/`
- OpenAI Audio Guide：`https://platform.openai.com/docs/guides/audio`
- OpenAI Realtime Transcription：`https://platform.openai.com/docs/guides/realtime-transcription`
