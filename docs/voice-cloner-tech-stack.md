# Voice Cloner 技术栈

本文定义 `voice-cloner` 桌面端前后端的推荐技术栈，基于当前已确认的产品与架构边界：

- 桌面端是主交互与本地注册表权威
- `FunSpeech` 是统一语音执行后端
- LLM 服务由桌面端直接调用本地模型服务
- 实时变声、ASR、TTS、Voice Design 最终都与 `FunSpeech` 对接

## 1. 总体技术栈

### 1.1 桌面应用总体

- 桌面壳：`Tauri 2`
- 前端：`Vue 3 + TypeScript + Vite`
- 桌面端后端：`Rust`
- 统一语音后端：`FunSpeech`
- 本地 LLM：桌面端直连本地模型服务

### 1.2 为什么这样选

- 当前仓库已经是 `Tauri + Vue` 骨架，延续成本最低
- `Rust` 更适合承接音频设备、缓冲、虚拟麦克风和本地桥接
- `FunSpeech` 已经具备 ASR / TTS / WebSocket / `voice_manager` 基础
- 你的边界已经明确，不需要再引入额外中间服务层

## 2. 前端技术栈

## 2.1 核心框架

- 框架：`Vue 3`
- 语言：`TypeScript`
- 构建工具：`Vite`
- 路由：`vue-router`
- 状态管理：`Pinia`

## 2.2 Tauri 前端桥接

- Tauri API：`@tauri-apps/api`

用途：

- 调用 Rust 命令
- 监听 Rust 端事件
- 与桌面能力打通

## 2.3 网络与数据交互

- HTTP 客户端：原生 `fetch`
- WebSocket：浏览器原生 `WebSocket`

理由：

- 当前接口数量有限
- MVP 不需要为了网络层再引入 `axios`
- `FunSpeech` 的 Realtime / WebSocket ASR / WebSocket TTS 都可直接适配

## 2.4 表单与校验

- 校验库：`zod`

建议用途：

- 设置页配置校验
- 音色设计输入校验
- 离线文本输入校验

## 2.5 样式层

建议顺序：

- 基础样式：原生 `CSS`
- 设计令牌：CSS Variables
- 组件样式分层：`main.css` + `tokens.css` + `components.css`

如需更高效率，可选：

- `UnoCSS`

MVP 不建议一开始引入重型 UI 库，原因是：

- 当前页面是强业务控制台，不是表单后台
- 设计还在快速变化，过早绑定组件库反而拖慢收敛

## 2.6 前端推荐依赖清单

```json
{
  "dependencies": {
    "vue": "^3.5.0",
    "vue-router": "^4.4.0",
    "pinia": "^2.3.0",
    "@tauri-apps/api": "^2.0.0",
    "zod": "^3.24.0"
  },
  "devDependencies": {
    "typescript": "^5.9.0",
    "vite": "^7.0.0",
    "@vitejs/plugin-vue": "^6.0.0",
    "vue-tsc": "^3.1.0"
  }
}
```

## 3. 桌面端后端技术栈

## 3.1 核心运行时

- 语言：`Rust`
- 桌面运行时：`Tauri 2`
- 异步运行时：`tokio`

## 3.2 通用能力库

- 序列化：`serde`, `serde_json`
- 错误处理：`thiserror`, `anyhow`
- 日志：`tracing`, `tracing-subscriber`
- 时间：`chrono` 或 `time`
- 文件路径：标准库 `std::path` 优先

## 3.3 网络通信

- HTTP 客户端：`reqwest`
- WebSocket 客户端：`tokio-tungstenite`

用途：

- 调 `FunSpeech` 的 REST 接口
- 连接 `FunSpeech` 的 WebSocket ASR / TTS / Realtime Voice
- 调本地 LLM 服务

## 3.4 本地存储

MVP 推荐：

- 本地配置：`JSON 文件`
- 本地音色注册表：`JSON 文件`
- 本地媒体文件：文件目录

后续如果数据复杂再升级：

- `SQLite`

原因：

- 你当前不要过度设计
- 本地音色注册表结构简单，`JSON + 文件目录` 足够快
- 更适合先跑通全量同步和增量同步

## 3.5 Rust 端推荐依赖清单

```toml
[dependencies]
tauri = { version = "2", features = [] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
reqwest = { version = "0.12", features = ["json", "multipart", "stream", "rustls-tls"] }
tokio-tungstenite = "0.24"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
```

## 4. 音频技术栈

这是桌面端最关键的一层。

## 4.1 音频采集与播放

- 音频设备枚举 / 采集 / 播放：`cpal`

用途：

- 麦克风采集
- 输出设备播放
- 本地监听

## 4.2 音频缓冲

- 环形缓冲：`ringbuf`

用途：

- 实时音频块缓冲
- 上下行流平滑
- 抖动隔离

## 4.3 音频文件

- WAV 读写：`hound`

如果需要更复杂格式处理：

- 优先交给 `FunSpeech`
- 或后续接 `ffmpeg`

## 4.4 虚拟麦克风

虚拟麦克风不建议依赖单一跨平台库硬解。

建议策略：

- Rust 层提供统一抽象
- Windows / macOS 做平台适配

即：

- Windows：WASAPI / 虚拟设备方案
- macOS：CoreAudio / 系统路由方案

## 4.5 音频层推荐依赖

```toml
[dependencies]
cpal = "0.15"
ringbuf = "0.4"
hound = "3.5"
```

## 5. FunSpeech 对接技术栈

`FunSpeech` 是统一语音执行后端，桌面端需要对接这些能力：

- REST ASR
- WebSocket ASR
- REST TTS
- OpenAI 兼容 TTS
- WebSocket TTS
- Realtime Voice
- Voice Design
- `voice_manager` 同步接口

桌面端 Rust 层建议分模块封装：

- `funspeech/asr.rs`
- `funspeech/tts.rs`
- `funspeech/realtime.rs`
- `funspeech/voice_design.rs`
- `funspeech/voice_sync.rs`

## 6. 本地 LLM 调用技术栈

你当前需求已经明确：

- LLM 服务在桌面端直接调用

推荐做法：

- 前端不直接请求本地模型服务
- Vue -> Tauri Rust -> 本地 LLM 服务

Rust 层使用：

- `reqwest`

原因：

- 更容易统一超时、重试和日志
- 更容易隐藏本地模型服务细节
- 更适合后续增加失败回退与诊断

## 7. 按模块划分的技术栈映射

## 7.1 音色管理

- 前端：`Vue 3`, `Pinia`
- 本地存储：`JSON 文件`
- 同步：`reqwest`
- 后端对接：`FunSpeech voice_manager`

## 7.2 实时变声

- 前端：`Vue 3`, `Pinia`
- Rust：`cpal`, `ringbuf`, `tokio`, `tokio-tungstenite`
- 后端对接：`FunSpeech Realtime Voice`

## 7.3 离线变声

- 前端：`Vue 3`, `Pinia`, 原生 `fetch`
- Rust：`reqwest`
- 后端对接：`FunSpeech ASR / TTS`

## 7.4 音色设计

- 前端：`Vue 3`, `Pinia`, `zod`
- Rust：`reqwest`
- 本地 LLM：桌面端本地模型服务
- 后端对接：`FunSpeech Voice Design (VoxCPM)`

## 8. 当前不建议引入的栈

当前阶段不建议：

- `Electron`
- 重型 UI 组件库
- `Axios`
- `gRPC`
- 过早引入 `SQLite`
- 复杂的 ORM
- 独立中间后端服务层

原因：

- 当前边界已经非常清晰
- 目标是先跑通音色同步、实时变声、离线变声、音色设计
- 过早上重栈会增加不必要复杂度

## 9. 一句话结论

`voice-cloner` 最合适的桌面端技术栈是：

- `Tauri 2 + Vue 3 + TypeScript + Vite + Pinia + Rust + tokio + reqwest + tokio-tungstenite + cpal + ringbuf`

配合：

- `FunSpeech` 作为统一语音执行后端
- 本地 `JSON + 文件目录` 作为音色注册表存储
- 桌面端直接调用本地 LLM 服务
