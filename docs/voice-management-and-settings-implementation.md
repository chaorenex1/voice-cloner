# 音色管理与设置实现文档

本文聚焦 `voice-cloner` 桌面端的两个基础模块：

- 音色管理
- 设置

目标不是抽象讨论，而是给出一份可以直接指导实现拆分的落地文档，覆盖：

1. 前端页面布局与线框图
2. Rust 侧文件组织、Tauri IPC 接口与数据结构

## 1. 当前仓库基线

先说明当前代码现实，避免文档脱离仓库：

- 前端目前只有一个骨架页面：`frontend/src/App.vue`
- 前端入口只有：`frontend/src/main.ts`
- 全局样式只有：`frontend/src/styles/main.css`
- Rust 端当前只有一个示例命令：`src-tauri/src/lib.rs` 中的 `get_app_summary`
- `src-tauri/src/main.rs` 只负责启动 Tauri

这意味着本次设计应按“新增模块结构”推进，而不是假设仓库里已经存在完整的 `pages/`、`stores/`、`services/`、`tauri/`、`models/` 体系。

同时，既有文档已经给出了产品边界：

- `docs/voice-cloner-mvp-prd.md`
- `docs/voice-cloner-technical-architecture.md`
- `docs/voice-cloner-flow-sequence-diagrams.md`
- `docs/voice-cloner-tech-stack.md`

本实现文档会沿用这些边界，但以当前 `Vue 3 + Tauri 2 + Rust` 骨架为真正落点。

## 2. 目标与范围

### 2.1 音色管理要解决什么

音色管理模块负责：

- 浏览本地可用音色
- 区分预置音色与自定义音色
- 查看单个音色详情
- 编辑自定义音色元信息
- 保存参考音频与参考文本
- 与 `FunSpeech voice_manager` 做全量/增量同步
- 处理同步失败、冲突、删除与重试

### 2.2 设置页要解决什么

设置模块负责：

- 输入/输出设备选择
- 虚拟麦克风选择与启用/停用
- `FunSpeech` 连接配置
- LLM / ASR / TTS / Realtime backend 配置
- 连接探测与错误反馈

### 2.3 不在本文展开的部分

- 实时变声链路内部音频处理
- 离线变声任务队列
- 音色设计工作流本身

这些能力会依赖音色管理和设置，但本文只定义这两个模块自己的实现边界。

## 3. 前端实现方案

## 3.1 先补的前端结构

建议从当前单页骨架演进到下面的结构：

```text
frontend/src/
  App.vue
  main.ts
  styles/
    main.css
  pages/
    VoiceLibraryPage.vue
    SettingsPage.vue
  components/
    shell/
      AppSidebar.vue
      AppTopbar.vue
      StatusPill.vue
    voice/
      VoiceLibraryToolbar.vue
      VoiceLibraryRail.vue
      VoiceLibraryList.vue
      VoiceLibraryCard.vue
      VoiceDetailPanel.vue
      VoicePreviewPlayer.vue
      VoiceEmptyState.vue
    settings/
      SettingsSectionTabs.vue
      DeviceSettingsForm.vue
      BackendSettingsForm.vue
      SettingsHealthPanel.vue
  stores/
    voice-library.store.ts
    settings.store.ts
  services/
    tauri/
      voice-library.ts
      settings.ts
      events.ts
  utils/
    types/
      voice.ts
      settings.ts
```

说明：

- `frontend/src/App.vue` 从“占位介绍页”收缩为应用壳
- 音色管理和设置页都走同一套桌面壳布局
- `services/tauri/*` 专门负责 `invoke` 和事件监听，不让页面直接写命令名
- `utils/types/*` 统一承接 Rust 返回的 camelCase 结构

### 为什么这样分

- 音色管理和设置都不是一次性向导，而是会被反复进入的常驻页面
- 左侧导航适合承载产品主模块
- 顶栏适合固定展示跨页面状态：连接状态、当前音色、快捷动作
- 内容区可以独立进化，不会把所有交互都堆进 `App.vue`

## 3.3 音色管理页布局

页面文件：

- `frontend/src/pages/VoiceLibraryPage.vue`

页面目标：

- 让用户在一个页面里完成浏览、试听、编辑、同步
- 让“当前音色是什么、参考文本和参考音频是什么、能否直接修改”一眼可见

更合理的桌面端方案不是把所有内容压成一页直铺，而是采用“主从双栏”：

1. 顶部操作条
2. 左侧音色浏览栏
3. 右侧当前音色详情舞台

这样更合适，原因是：

- 音色管理的主操作是“找音色、试听、切换”，不是连续填写表单
- 列表与详情同时可见，能减少来回返回列表的打断感
- 右侧详情区可以做得更舒展，参考文本和波形区会更美观
- 现有仓库的玻璃卡片视觉本身就更适合双栏舞台，而不是长页面堆控件

### 线框图 A：音色管理默认态

```text
+----------------------------------------------------------------------------------------------------------------+
| 音色管理                                                            [新增音色] [从云端同步] [刷新]             |
| 搜索 [温柔女声________________________________________]                                                          |
+------------------------------------------+---------------------------------------------------------------------+
| 音色浏览栏                               | 当前音色舞台                                                        |
| ---------------------------------------- | ------------------------------------------------------------------- |
| 温柔女声                                 | [ 封面 / 波形视觉区 ]                                                |
| [试听] [设为当前音色]                    |                                                                     |
|                                          | 名称: 温柔女声                                                      |
| 少年音                                   | [试听] [保存修改] [删除]                                            |
| [试听] [设为当前音色]                    |                                                                     |
|                                          | 参考文本                                                            |
| 机械音                                   | [ textarea....................................................... ] |
| [试听] [设为当前音色]                    |                                                                     |
|                                          | 参考音频                                                            |
| 电台男声                                 | [waveform......................................................]    |
| [试听] [设为当前音色]                    | [重新上传] [清除]                                                   |
|                                          |                                                                     |
|                                          | 底部状态条: 当前音色已载入，可直接用于实时变声                      |
+------------------------------------------+---------------------------------------------------------------------+
```

### 线框图 B：新增自定义音色

```text
+----------------------------------------------------------------------------------------------------------------+
| 音色管理 / 新增自定义音色                                                               [取消]   |
+------------------------------------------+---------------------------------------------------------------------+
| 左侧说明区                               | 新建表单                                                            |
| - 命名建议                               | 名称 *                                                              |
| - 录音建议                               | [____________________________________]                              |
| - 文本长度建议                           |                                                                     |
|                                          | 参考文本 *                                                          |
|                                          | [ textarea....................................................... ] |
|                                          |                                                                     |
|                                          | 参考音频 *                                                          |
|                                          | [上传按钮] [录音入口]                                               |
|                                          |                                                                     |
|                                          | [保存音色]                                                          |
+------------------------------------------+---------------------------------------------------------------------+
```

### 页面区块职责

#### 顶部工具条

建议组件：

- `VoiceLibraryToolbar.vue`

职责：

- 搜索
- 新增音色
- 触发全量/增量同步

#### 左侧音色浏览栏

建议组件：

- `VoiceLibraryRail.vue`
- `VoiceLibraryList.vue`
- `VoiceLibraryCard.vue`

职责：

- 承接高频浏览与切换
- 支持试听
- 支持设置当前音色
- 支持进入编辑态

#### 右侧当前音色舞台

建议组件：

- `VoiceDetailPanel.vue`
- `VoicePreviewPlayer.vue`

职责：

- 查看和编辑当前选中音色
- 试听
- 上传或替换参考音频
- 删除音色
- 承载视觉中心，避免列表和表单互相打架

## 3.4 设置页布局

页面文件：

- `frontend/src/pages/SettingsPage.vue`

设置页更适合采用“顶部分段切换 + 双卡片内容区 + 底部操作条”，而不是一页长表单。

这样更合理，原因是：

- 设置属于低频操作，分段切换比长表单更好扫读
- 当前只剩“设备设置”和“后端设置”两块，正好适合分段
- 双卡片内容区更符合当前玻璃面板视觉，也会更整洁
- 固定的底部操作条能让“测试连接 / 保存设置”始终稳定可见

### 线框图 C：设置页默认态

```text
+----------------------------------------------------------------------------------------------------------------+
| 设置                                                                                              |
| [设备设置] [后端设置]                                                                             |
+------------------------------------------------+---------------------------------------------------+
| 设备主卡                                                                               |
| 输入设备 [Shure MV7____________________v]                                      |
| 输出设备 [Headphones___________________v]                                              |
| 虚拟麦克风 [VB-Cable Input_____________v]                                              |
| [x] 启用虚拟麦克风                                                                      |
+------------------------------------------------+---------------------------------------------------+
```

### 线框图 D：后端连接设置

```text
+----------------------------------------------------------------------------------------------------------------+
| 设置 / 后端设置                                                                                  |
| [设备设置] [后端设置]                                                                             |
+------------------------------------------------+---------------------------------------------------+
| FunSpeech 卡                                    | LLM / 语音后端卡                                   |
| Base URL [http://127.0.0.1:8000___________]     | LLM Base URL [http://127.0.0.1:11434__________]    |
| API Key Ref [local/funspeech/default______]     | LLM Model    [qwen2.5:latest__________________]    |
| Timeout(ms) [10000________________________]     |                                                   |
+------------------------------------------------+---------------------------------------------------+
```

### 设置布局建议

#### 1. 顶部分段切换

- `设备设置`
- `后端设置`

#### 2. 设备设置页

- 输入设备
- 输出设备
- 虚拟麦克风设备
- 虚拟麦克风启用/停用
- 右侧连接状态卡

#### 3. 后端设置页

- `FunSpeech`
- LLM

## 3.5 前端状态拆分

### `voice-library.store.ts`

建议字段：

```ts
export interface VoiceLibraryState {
  voices: VoiceSummary[];
  selectedVoiceName: string | null;
  detail: VoiceDetail | null;
  search: string;
  loading: boolean;
  saving: boolean;
}
```

### `settings.store.ts`

建议字段：

```ts
export interface SettingsState {
  settings: AppSettings | null;
  audioDevices: AudioDeviceSnapshot | null;
  health: BackendHealthSnapshot[];
  loading: boolean;
  saving: boolean;
}
```

## 3.6 前端与 Tauri 的桥接层

页面不要直接写 `invoke('xxx')`，统一下沉到：

- `frontend/src/services/tauri/voice-library.ts`
- `frontend/src/services/tauri/settings.ts`
- `frontend/src/services/tauri/events.ts`

这样后面命令名或返回结构变动时，影响面最小。

## 4. Rust 侧实现方案

## 4.1 推荐目录结构

当前 `src-tauri/src/` 只有 `lib.rs` 和 `main.rs`，建议补成下面的结构：

```text
src-tauri/src/
  main.rs
  lib.rs
  state/
    app_state.rs
  tauri/
    mod.rs
    voice_library.rs
    settings.rs
  models/
    mod.rs
    voice.rs
    settings.rs
    common.rs
  services/
    mod.rs
    voice_registry_service.rs
    voice_sync_service.rs
    settings_service.rs
    healthcheck_service.rs
  storage/
    mod.rs
    paths.rs
    json_store.rs
  funspeech/
    mod.rs
    voice_manager.rs
  events/
    mod.rs
    app_events.rs
  error/
    mod.rs
    app_error.rs
  utils/
    mod.rs
    fs.rs
    time.rs
```

职责边界：

- `tauri/`：Tauri IPC 暴露层
- `services/`：业务逻辑
- `storage/`：本地 JSON / 文件目录读写
- `funspeech/`：对外部后端接口的 HTTP 适配
- `models/`：前后端共享的序列化结构
- `events/`：Rust -> 前端事件桥接

## 4.2 `lib.rs` 的角色

`src-tauri/src/lib.rs` 保持为应用装配根，不承载业务细节。

建议只负责：

- 初始化 `AppState`
- 注册 Tauri commands
- 注册共享 service
- 后续如有需要，启动配置目录检查

建议形态：

```rust
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            tauri::voice_library::list_voices,
            tauri::voice_library::get_voice_detail,
            tauri::voice_library::create_voice,
            tauri::voice_library::update_voice,
            tauri::voice_library::delete_voice,
            tauri::voice_library::sync_voices,
            tauri::settings::get_settings,
            tauri::settings::update_settings,
            tauri::settings::list_audio_devices,
            tauri::settings::check_backend_health,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## 4.3 本地文件布局

沿用架构文档里已经约定的路径：

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

额外建议补一个同步状态文件：

```text
~/voice-cloner/
  library/
    sync-state.json
```

用途：

- 记录上次全量同步时间
- 记录远端版本戳或摘要
- 记录失败项

## 4.4 数据模型建议

### `models/voice.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSummary {
    pub voice_name: String,
    pub display_name: String,
    pub has_reference_audio: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceDetail {
    pub voice_name: String,
    pub display_name: String,
    pub voice_instruction: Option<String>,
    pub reference_text: String,
    pub reference_audio_path: Option<String>,
    pub preview_audio_path: Option<String>,
}
```

### `models/settings.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub devices: DeviceSettings,
    pub backends: BackendSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub virtual_mic_device_id: Option<String>,
    pub virtual_mic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendEndpointConfig {
    pub provider_name: String,
    pub base_url: String,
    pub api_key_ref: Option<String>,
    pub model: Option<String>,
    pub timeout_ms: u64,
    pub region: Option<String>,
    pub extra_options: serde_json::Value,
}
```

## 4.5 命令层接口

## 4.5.1 音色管理命令

文件：

- `src-tauri/src/tauri/voice_library.rs`

建议暴露这些命令：

### `list_voices`

```rust
#[tauri::command]
async fn list_voices(state: tauri::State<'_, AppState>) -> Result<Vec<VoiceSummary>, AppError>
```

用途：

- 页面首屏列表
- 搜索刷新后的重新拉取

### `get_voice_detail`

```rust
#[tauri::command]
async fn get_voice_detail(
    voice_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<VoiceDetail, AppError>
```

用途：

- 点击列表项后加载右侧详情

### `create_voice`

```rust
#[tauri::command]
async fn create_voice(
    input: CreateVoiceInput,
    state: tauri::State<'_, AppState>,
) -> Result<VoiceMutationResult, AppError>
```

`CreateVoiceInput` 建议字段：

- `voiceName`
- `displayName`
- `referenceText`
- `referenceAudioPath`
- `voiceInstruction`

### `update_voice`

```rust
#[tauri::command]
async fn update_voice(
    input: UpdateVoiceInput,
    state: tauri::State<'_, AppState>,
) -> Result<VoiceMutationResult, AppError>
```

用途：

- 编辑自定义音色
- 替换参考音频/文本

### `delete_voice`

```rust
#[tauri::command]
async fn delete_voice(
    voice_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<VoiceMutationResult, AppError>
```

规则：

- 预置音色不允许本地删除
- 自定义音色删除时同时删本地 JSON / WAV
- 如已同步到远端，优先尝试远端删除；失败则记录错误并在下次同步重试

### `sync_voices`

```rust
#[tauri::command]
async fn sync_voices(
    request: SyncVoicesRequest,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<VoiceSyncResult, AppError>
```

`SyncVoicesRequest` 建议字段：

- `mode`: `full | incremental | retryFailed`
- `voiceNames`: `Option<Vec<String>>`

这个命令需要事件回推进度。

## 4.5.2 设置命令

文件：

- `src-tauri/src/tauri/settings.rs`

### `get_settings`

```rust
#[tauri::command]
async fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, AppError>
```

用途：

- 设置页首屏
- 启动时恢复上次配置

### `update_settings`

```rust
#[tauri::command]
async fn update_settings(
    input: UpdateSettingsInput,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, AppError>
```

建议支持“按 section 更新”，避免每次整包覆盖。

`UpdateSettingsInput` 建议字段：

- `section`: `devices | backends`
- `payload`: `serde_json::Value`

### `list_audio_devices`

```rust
#[tauri::command]
async fn list_audio_devices() -> Result<AudioDeviceSnapshot, AppError>
```

返回：

- 输入设备列表
- 输出设备列表
- 虚拟麦克风设备列表

### `check_backend_health`

```rust
#[tauri::command]
async fn check_backend_health(
    request: BackendHealthRequest,
    state: tauri::State<'_, AppState>,
) -> Result<BackendHealthResult, AppError>
```

用途：

- 设置页点击“测试连接”
- 保存前主动验证

## 4.6 事件接口

如果同步和健康检查只靠命令返回，前端会缺实时反馈。建议增加事件：

文件：

- `src-tauri/src/events/app_events.rs`
- `frontend/src/services/tauri/events.ts`

建议事件名：

- `voice-library-updated`
- `voice-sync-progress`
- `voice-sync-finished`
- `settings-updated`
- `backend-health-updated`

### `voice-sync-progress` 示例

```json
{
  "stage": "push_remote",
  "voiceName": "radio_male",
  "processed": 3,
  "total": 8,
  "message": "uploading reference audio"
}
```

### `backend-health-updated` 示例

```json
{
  "service": "funspeech",
  "status": "ok",
  "latencyMs": 83,
  "message": "reachable"
}
```

## 4.7 Service 层职责

### `voice_registry_service.rs`

负责：

- 读取本地音色目录
- 生成 `VoiceSummary`
- 读取/写入单个音色详情
- 本地删除

### `voice_sync_service.rs`

负责：

- 首次全量同步
- 按 `voice_name` 增量同步
- 删除远端音色
- 写入 `sync-state.json`
- 发出同步进度事件

### `settings_service.rs`

负责：

- 读写 `app-settings.json`
- 合并 section 更新
- 提供默认配置

### `healthcheck_service.rs`

负责：

- 测试 `FunSpeech`、LLM、TTS、ASR、Realtime 地址
- 统一超时与错误结构

## 4.8 与 FunSpeech 的接口边界

文件：

- `src-tauri/src/funspeech/voice_manager.rs`

建议只保留“HTTP 适配”职责，不要混入本地文件逻辑。

推荐方法：

- `fetch_remote_voices()`
- `register_remote_voice()`
- `update_remote_voice()`
- `delete_remote_voice()`
- `refresh_remote_voices()`

这样 `voice_sync_service.rs` 才是真正的同步 orchestration 层。

## 5. 前后端契约建议

## 5.1 TypeScript 契约文件

建议新增：

- `frontend/src/utils/types/voice.ts`
- `frontend/src/utils/types/settings.ts`

这些结构要对齐 Rust 的 camelCase 序列化输出，不要在页面层自己拼字段。

## 5.2 错误返回建议

前端页面最需要的是“能展示给用户”的错误，而不是一串调试堆栈。

建议统一返回：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorPayload {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}
```

常见错误码建议：

- `VOICE_NOT_FOUND`
- `VOICE_NAME_CONFLICT`
- `REFERENCE_AUDIO_MISSING`
- `REMOTE_SYNC_FAILED`
- `DEVICE_NOT_FOUND`
- `BACKEND_UNREACHABLE`
- `SETTINGS_VALIDATION_FAILED`

## 6. 实现顺序建议

按风险和依赖顺序，建议分 4 步：

### 第 1 步：补前端结构和静态页

- 把 `App.vue` 收缩成应用壳
- 新增 `VoiceLibraryPage.vue`
- 新增 `SettingsPage.vue`
- 先用 mock 数据跑通布局与交互骨架

### 第 2 步：补 Rust 本地读写

- 建 `models/`
- 建 `storage/`
- 建 `settings_service.rs`
- 建 `voice_registry_service.rs`
- 先打通本地 JSON 读写，不接远端同步

### 第 3 步：补 Tauri 命令与前端桥接

- 建 `tauri/voice_library.rs`
- 建 `tauri/settings.rs`
- 前端改为通过 `services/tauri/*` 调用

### 第 4 步：接 FunSpeech 同步与健康检查

- 建 `funspeech/voice_manager.rs`
- 建 `voice_sync_service.rs`
- 建 `healthcheck_service.rs`
- 接事件推送

## 7. 最小可交付版本

如果要先做一个可用版本，而不是一次做完，最小闭环建议是：

### 音色管理 MVP

- 列表展示
- 详情查看
- 新增自定义音色
- 编辑参考文本
- 上传参考音频
- 保存到本地
- 手动同步到 `FunSpeech`

### 设置 MVP

- 输入/输出设备选择
- 虚拟麦克风设备选择
- 虚拟麦克风启用/停用
- `FunSpeech` base URL 配置
- LLM base URL / model 配置
- 保存本地设置
- 测试连接

## 8. 一句话结论

这两个模块最合适的落地方式是：

- 前端采用“桌面壳 + 页面化 + store + tauri service”的结构
- 音色管理页采用“单页操作区 + 列表区 + 编辑区”的布局
- 设置页采用“单页设备区 + 单页后端区”的布局
- Rust 端采用“tauri / services / storage / funspeech / models” 分层
- Tauri 只暴露少量稳定 IPC，业务逻辑全部放到 service 层
