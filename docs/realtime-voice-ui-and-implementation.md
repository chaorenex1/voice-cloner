# Realtime Voice 前端界面与实现设计

本文专注于 `voice-cloner` 的实时变声模块，覆盖两部分：

- 实时变声前端界面设计
- 基于当前时序图与架构约束的实现技术细节

当前前提：

- 实时变声统一接入 `FunSpeech Realtime Voice`
- 桌面端本地音色注册表是产品权威源
- 首次启动从 `FunSpeech voice_manager` 全量同步，后续新增 / 修改 / 删除增量同步
- LLM 不参与实时变声主链路

## 1. 模块目标

实时变声模块的目标不是“展示很多参数”，而是让用户在最短路径内完成这 4 件事：

1. 选中一个可用音色
2. 确认输入设备正常
3. 一键开始实时变声
4. 把结果稳定送到监听与虚拟麦克风

因此 UI 设计要服务两个优先级：

- `优先级 1`：状态清楚
- `优先级 2`：操作快

## 2. 界面结构设计

## 2.1 页面定位

页面文件建议：

- `frontend/src/pages/RealtimeVoicePage.vue`

它应该是桌面端默认主页面，而不是一个次级配置页。

## 2.2 页面布局

建议采用“单屏双区 + 轻量页头”布局，不要做三段式结构，也不要堆很多 Tab。

```text
+----------------------------------------------------------------------------------+
| 页头：当前音色 / 连接状态 / 输入设备 / 输出设备 / 延迟 / 设置入口               |
+----------------------------------------------------------------------------------+
| 左侧中心舞台                            | 右侧控制侧栏                           |
| - 当前音色大卡                          | - 音色列表                             |
| - 开始 / 停止主按钮                     | - 常用参数                             |
| - 输入电平 / 输出状态 / 虚拟麦克风状态  | - 高级设置（折叠）                     |
| - 状态提示 / 重连动作                   | - 同步状态 / 监听开关                  |
+----------------------------------------------------------------------------------+
```

## 2.3 区域职责

### 中心舞台

用途：

- 把“当前是否能说、当前声音是谁、当前链路是否正常”放在视觉中心
- 承担最核心的开始 / 停止操作
- 承担最重要的运行态反馈

必须展示：

- `voice_name`
- 当前实时状态
- 开始 / 停止大按钮
- 输入电平
- 虚拟麦克风状态
- 当前延迟
- 当前状态提示

建议组件：

- `RealtimeControlBar.vue`
- `AudioMeter.vue`
- `RealtimeStatusBadge.vue`
- `VirtualMicStatus.vue`

### 控制侧栏

用途：

- 把高频控制项集中到一个稳定位置
- 让用户在同一视线范围内完成音色切换和参数调整
- 用折叠分组代替多 Tab 切换

建议放：

- 音色列表
- 监听开关
- 常用参数
- 高级设置
- 音色同步状态

建议组件：

- `RealtimeSidebarPanel.vue`
- `VoiceList.vue`
- `VoiceCard.vue`
- `VoiceParamsPanel.vue`
- `VoiceSyncStatus.vue`

## 2.4 页头信息

页头不应只放标题，建议直接承担“实时运行摘要”。

建议展示：

- 当前 `voice_name`
- 输入设备名称
- 输出设备名称
- `FunSpeech Realtime Voice` 连接状态
- 当前延迟（如 `92ms`）
- 设置入口

## 2.5 控制侧栏的信息密度建议

控制侧栏适合承载次重要信息，但不应该抢中心舞台的注意力。

建议展示：

- 默认展开 `音色` 与 `常用参数`
- `高级设置` 默认折叠
- 监听开关与同步状态固定在侧栏顶部或底部
- 同类控制按分组堆叠，避免用多层 Tab 让用户来回切换

## 2.6 页面线框图说明

这里不画高保真视觉稿，而是给出可直接落组件的低保真线框说明。

### 线框图 A：默认待机态

```text
+----------------------------------------------------------------------------------+
| Voice Cloner                                         FunSpeech 已连接   92ms     |
| 输入设备: Shure MV7        输出设备: Headphones      虚拟麦克风: 就绪   [设置]  |
+----------------------------------------------------------------------------------+
| [ 当前音色头像/封面 ]                       | 音色                                  |
| 温柔女声 / preset                           | - 温柔女声 (当前)                     |
| 状态: 待机中，可开始实时变声                | - 少年音                              |
| 输入电平  [----|-----]                      | - 机械音                              |
| 输出状态  未开始                            | ------------------------------------ |
| [ 开始实时变声 ]   [ 试听音色 ]             | 常用参数                              |
|                                            | 音高      [----o-----]               |
|                                            | 强度      [------o---]               |
|                                            | 亮度      [---o------]               |
|                                            | ------------------------------------ |
|                                            | 高级设置（折叠）                     |
|                                            | 监听: 关   同步状态: 已同步          |
+----------------------------------------------------------------------------------+
```

说明：

- 用户打开页面时，注意力首先落在“当前音色 + 开始按钮”
- 顶栏直接展示设备和连接摘要，不需要先点设置才能知道状态
- 侧栏把音色和常用参数固定在同一屏，避免来回切换 Tab

### 线框图 B：运行态

```text
+----------------------------------------------------------------------------------+
| Voice Cloner                                         FunSpeech 运行中   86ms     |
| 输入设备: Shure MV7        输出设备: Headphones      虚拟麦克风: 输出中 [设置]  |
+----------------------------------------------------------------------------------+
| [ 当前音色头像/封面 ]                       | 音色                                  |
| 机械音 / custom                             | - 温柔女声                            |
| 状态: 实时变声运行中                        | - 少年音                              |
| 输入电平   [||||||||--]                     | - 机械音 (当前)                       |
| 输出流状态: 正常    音频块速率: 50fps      | ------------------------------------ |
| [ 停止 ]   [ 静音监听 ]   [ 切换音色 ]      | 常用参数                              |
|                                            | 音高      [----o-----]               |
|                                            | 强度      [------o---]               |
|                                            | 亮度      [---o------]               |
|                                            | 淡入淡出  [--o-------]               |
|                                            | [恢复默认]                            |
|                                            | 监听: 开   同步状态: 已同步          |
+----------------------------------------------------------------------------------+
```

说明：

- 运行态下主按钮必须从“开始”切成“停止”，且视觉优先级最高
- 输入电平、虚拟麦克风状态、延迟必须同时可见
- 参数区在运行态始终就地可调，但不应挤占中心舞台

### 线框图 C：切换音色中

```text
+----------------------------------------------------------------------------------+
| Voice Cloner                                         FunSpeech 切换中   104ms    |
| 输入设备: Shure MV7        输出设备: Headphones      虚拟麦克风: 暂稳态 [设置]  |
+----------------------------------------------------------------------------------+
| [ 新目标音色封面 ]                          | 音色                                  |
| 少年音 / preset                             | - 温柔女声                            |
| 状态: 正在切换音色，请稍候                  | - 少年音 (切换中)                     |
| 输入电平   [|||||-----]                     | - 机械音 (旧音色)                     |
| 输出流状态: 切换中                          | ------------------------------------ |
| [ 停止 ]   [取消切换不可用]                 | 常用参数（部分锁定）                  |
| 提示: 后端确认后将应用新音色                | 音高      [----o-----]               |
|                                            | 强度      [------o---]               |
|                                            | 高级设置（锁定）                     |
|                                            | 监听: 开   同步状态: 已同步          |
+----------------------------------------------------------------------------------+
```

说明：

- 切换音色是一个短暂过渡态
- 过渡期可以保留停止按钮，但应锁定部分参数操作
- 必须避免用户误以为点击后已经立即生效

### 线框图 D：错误/重连态

```text
+----------------------------------------------------------------------------------+
| Voice Cloner                                         FunSpeech 断开      --      |
| 输入设备: Shure MV7        输出设备: Headphones      虚拟麦克风: 已暂停 [设置]  |
+----------------------------------------------------------------------------------+
| [ 当前音色封面灰化 ]                         | 音色                                  |
| 机械音 / custom                              | - 当前音色保留显示                    |
| 状态: 连接中断，正在尝试恢复                 | ------------------------------------ |
| 输入电平   [||--------]                      | 常用参数（禁用）                      |
| 输出流状态: 已暂停                           | 音高      [----o-----]               |
| [ 重试连接 ]   [ 停止 ]                      | 强度      [------o---]               |
| 恢复后继续输出到虚拟麦克风                   | ------------------------------------ |
|                                             | 高级设置（折叠）                     |
|                                             | 监听: 关   同步状态: 未受影响        |
+----------------------------------------------------------------------------------+
```

说明：

- 出错时不要把页面直接清空成空白
- 应保留当前音色和可恢复动作，让用户知道当前处于什么状态
- 不额外打开调试区，避免把故障态变成调试台

## 2.7 组件落点映射

将线框图映射到组件，大致如下：

### 顶栏

- `RealtimeHeaderBar.vue`
  - 当前连接状态
  - 输入输出设备
  - 虚拟麦克风状态
  - 延迟
  - 设置入口

### 中心舞台

- `RealtimeStage.vue`
  - 音色大卡
  - 状态文案
  - 输入电平
  - 输出流状态
  - 主按钮区
  - 状态提示

### 控制侧栏

- `RealtimeSidebarPanel.vue`
  - `VoiceList.vue`
  - `VoiceParamsPanel.vue`
  - `RealtimeAdvancedPanel.vue`
  - `VoiceSyncStatus.vue`

## 2.8 页面响应规则

### 侧栏默认规则

- Idle：默认展开 `音色` 与 `常用参数`
- Running：记住用户上次展开的分组状态
- Error：保留 `音色` 可见，禁用不可编辑参数，`高级设置` 默认折叠

### 中心按钮规则

- Idle：`开始实时变声`
- Preparing：`连接中...`
- Running：`停止`
- Reconnecting：`重试连接`
- Error：`重试连接`

### 参数可编辑规则

- Idle：可编辑
- Preparing：禁用
- Running：可编辑
- Switching：部分锁定
- Error：禁用

## 3. 页面状态设计

实时变声页至少要有 6 个状态：

### 3.1 Idle

- 未开始
- 可切换音色
- 可调整参数
- 主按钮显示“开始实时变声”

### 3.2 Preparing

- 正在连接 `FunSpeech Realtime Voice`
- 正在检查设备
- 主按钮禁用
- 显示加载反馈

### 3.3 Running

- 正在实时变声
- 输入电平动态显示
- 参数可热更新
- 音色允许切换，但有短暂“切换中”状态

### 3.4 Reconnecting

- 网络抖动或服务重连
- 暂停虚拟麦克风写入
- 给出可恢复提示

### 3.5 Error

- 启动失败 / 设备错误 / 推理错误
- 主按钮回退为可重试
- 展示恢复提示和建议动作

### 3.6 Muted / No Input

- 会话仍在，但当前没有有效输入
- 维持运行态
- 明确提示“未检测到语音输入”

## 4. 关键交互设计

## 4.1 开始实时变声

点击主按钮后：

1. 校验输入设备
2. 校验当前音色是否可用
3. 建立 `FunSpeech Realtime Voice` 会话
4. 进入 `Preparing`
5. 成功后进入 `Running`

## 4.2 停止实时变声

点击停止后：

1. 停止采集
2. 停止发送音频块
3. 关闭 WebSocket
4. 清空本地 buffer
5. 停止虚拟麦克风输出
6. 回到 `Idle`

## 4.3 运行中切换音色

建议保留在运行中切换，但要明确这是“热切换”，不是无感切换。

交互建议：

1. 用户点新音色
2. UI 显示“切换中”
3. 发 `updateVoice` 给 `FunSpeech`
4. 成功后替换当前 `voice_name`
5. 失败则回退到旧音色

## 4.4 运行中调参数

参数调节建议分两种更新策略：

- 滑动中只更新本地 UI
- 停止拖动后再把最终值推给后端

原因：

- 降低频繁网络抖动
- 减少无意义实时请求

对极少数需要低延迟反馈的参数，可支持节流热更新。

## 5. 前端组件与状态拆分

## 5.1 推荐组件树

```text
RealtimeVoicePage
├─ RealtimeHeaderBar
└─ RealtimeWorkspace
   ├─ RealtimeStage
   │  ├─ RealtimeStatusBadge
   │  ├─ AudioMeter
   │  ├─ RealtimeControlBar
   │  └─ VirtualMicStatus
   └─ RealtimeSidebarPanel
      ├─ VoiceList
      │  └─ VoiceCard[]
      ├─ VoiceParamsPanel
      ├─ RealtimeAdvancedPanel
      └─ VoiceSyncStatus
```

## 5.2 推荐状态拆分

建议主要状态放入：

- `realtime.store.ts`
- `voice-library.store.ts`
- `settings.store.ts`

### `realtime.store.ts`

建议字段：

- `status`
- `sessionId`
- `currentVoiceName`
- `runtimeParams`
- `inputLevel`
- `latencyMs`
- `websocketState`
- `virtualMicState`
- `lastError`
- `statusHint`

### `voice-library.store.ts`

建议字段：

- `voices`
- `selectedVoiceName`
- `syncStatus`
- `lastSyncedAt`

### `settings.store.ts`

建议字段：

- `inputDeviceId`
- `outputDeviceId`
- `funspeechRealtimeBaseUrl`
- `monitorEnabled`
- `virtualMicEnabled`

## 6. 变声实现技术细节

## 6.1 实时链路分层

实时变声实现建议分成 5 层：

### 1. 设备层

Rust 负责：

- 采集麦克风
- 枚举输入输出设备
- 启停监听播放
- 虚拟麦克风写入

建议文件：

- `src-tauri/src/audio/capture.rs`
- `src-tauri/src/audio/monitor.rs`
- `src-tauri/src/audio/virtual_mic.rs`
- `src-tauri/src/audio/devices.rs`

### 2. 缓冲层

Rust 负责：

- 输入 buffer
- 上行发送队列
- 下行回放队列
- 抖动处理

建议文件：

- `src-tauri/src/audio/buffer.rs`

### 3. 会话层

Rust 负责：

- 建立 WebSocket
- 发送开始 / 停止 / 更新参数 / 切换音色
- 管理连接状态

建议文件：

- `src-tauri/src/funspeech/realtime.rs`
- `src-tauri/src/commands/realtime.rs`

### 4. 状态桥接层

Rust -> 前端事件：

- `session_ready`
- `input_level_changed`
- `latency_updated`
- `voice_updated`
- `stream_error`
- `session_closed`

建议文件：

- `src-tauri/src/events/realtime_events.rs`
- `frontend/src/services/tauri/events.ts`

### 5. UI 状态层

前端负责：

- 运行态渲染
- 参数编辑
- 错误展示
- 角色切换和状态回退

## 6.2 WebSocket 会话协议建议

建议实时 WebSocket 消息按类型分开，而不是一条消息塞所有字段。

### 客户端 -> FunSpeech

```json
{ "type": "start", "voice_name": "robot_a", "params": { "pitch": 0.1, "brightness": 0.3 } }
{ "type": "audio_chunk", "seq": 1, "pcm": "<binary-or-base64>" }
{ "type": "update_params", "params": { "pitch": 0.2 } }
{ "type": "update_voice", "voice_name": "girl_b" }
{ "type": "stop" }
```

### FunSpeech -> 客户端

```json
{ "type": "ready", "session_id": "rt_001" }
{ "type": "audio_chunk", "seq": 1, "pcm": "<binary-or-base64>" }
{ "type": "latency", "value_ms": 96 }
{ "type": "voice_updated", "voice_name": "girl_b" }
{ "type": "error", "code": "VOICE_NOT_FOUND", "message": "voice not loaded" }
{ "type": "closed" }
```

## 6.3 音频块策略

实时链路成败高度依赖音频块大小。

建议：

- 初始块长：`20ms`
- 必要时试 `40ms`
- 不建议一开始就上太大 chunk

原因：

- `20ms` 更利于压低交互延迟
- chunk 太大时延迟会上升
- chunk 太小会放大包开销和抖动敏感度

## 6.4 输入输出处理建议

### 输入侧

- 采样率统一
- 单声道化
- 基础音量检测
- 可选静音门限

### 输出侧

- 单独监听队列
- 单独虚拟麦克风队列
- 断流时立即停写，不输出旧 buffer 脏数据

## 6.5 运行中切换参数的实现策略

推荐策略：

- UI 滑动时只更新本地显示
- `150ms ~ 250ms` 节流后再发 `update_params`
- 只有最终值入后端

适合热更新的参数：

- `pitch`
- `strength`
- `brightness`

不建议高频热更新的参数：

- 会导致后端重装或重绑定的参数
- 会影响 buffer 策略的参数

## 6.6 运行中切换音色的实现策略

这是高风险动作，建议这样做：

1. UI 进入 `switching` 子状态
2. 暂停显示“新音色已生效”直到后端 ack
3. 后端成功后替换当前音色
4. 失败则恢复旧音色与旧参数

前端一定要保存：

- `previousVoiceName`
- `previousRuntimeParams`

## 6.7 延迟监控与展示

前端页面不能只写“低延迟”，必须有真实可见反馈。

建议实时显示：

- 当前延迟 `latencyMs`
- 输入电平
- WebSocket 状态
- 丢包 / 重连状态

颜色建议：

- `<100ms`：绿色
- `100-150ms`：黄色
- `>150ms`：红色

## 6.8 错误处理设计

### 启动前错误

- 无输入设备
- 当前音色未同步
- FunSpeech 地址不可达

处理：

- 阻止进入 `Preparing`
- 在按钮旁直接给出阻断提示

### 运行中错误

- WebSocket 断开
- 后端返回 `voice not found`
- 音频写入失败

处理：

- 停止虚拟麦克风输出
- 清空 buffer
- UI 进入 `Error` 或 `Reconnecting`

### 切换错误

- 音色切换失败
- 参数更新失败

处理：

- 局部回退
- 不强制结束整个会话

## 7. 建议的前端文件清单

至少建议新增这些文件：

- `frontend/src/pages/RealtimeVoicePage.vue`
- `frontend/src/components/realtime/RealtimeSidebarPanel.vue`
- `frontend/src/components/realtime/RealtimeControlBar.vue`
- `frontend/src/components/realtime/AudioMeter.vue`
- `frontend/src/components/realtime/RealtimeStatusBadge.vue`
- `frontend/src/components/realtime/VirtualMicStatus.vue`
- `frontend/src/components/realtime/RealtimeAdvancedPanel.vue`
- `frontend/src/components/voice/VoiceList.vue`
- `frontend/src/components/voice/VoiceCard.vue`
- `frontend/src/components/voice/VoiceParamsPanel.vue`
- `frontend/src/components/voice/VoiceSyncStatus.vue`
- `frontend/src/stores/realtime.store.ts`
- `frontend/src/services/api/realtime-api.ts`
- `frontend/src/composables/useRealtimeVoice.ts`
- `frontend/src/types/realtime.ts`

Rust 端至少建议新增：

- `src-tauri/src/commands/realtime.rs`
- `src-tauri/src/audio/capture.rs`
- `src-tauri/src/audio/monitor.rs`
- `src-tauri/src/audio/virtual_mic.rs`
- `src-tauri/src/audio/buffer.rs`
- `src-tauri/src/funspeech/realtime.rs`
- `src-tauri/src/events/realtime_events.rs`

## 8. 一句话结论

实时变声模块最重要的不是“参数多”，而是：

- `状态清楚`
- `开始够快`
- `切换可回退`
- `断流不脏输出`

前端界面设计应围绕运行态控制台来展开；实现上则应把音频设备、缓冲、WebSocket 会话、状态桥接和 UI 状态严格分层。
