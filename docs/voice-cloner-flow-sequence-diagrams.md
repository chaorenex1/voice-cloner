# Voice Cloner 流程图与时序图

本文为 MVP 四个核心模块补充流程图与时序图：

- 音色管理
- 实时变声
- 离线变声
- 音色设计

图示基于当前 PRD 与技术架构约束：

- 桌面端是主交互与本地注册表权威
- `FunSpeech` 负责 ASR / TTS / Realtime Voice / Voice Design / `voice_manager`
- LLM 服务由桌面端直接调用本地模型服务
- 首次启动从 `FunSpeech voice_manager` 全量同步，后续新增 / 修改 / 删除走增量同步

## 1. 音色管理

### 1.1 流程图

```mermaid
flowchart TD
    A[首次启动或进入音色管理] --> B{是否首次启动}
    B -- 是 --> C[从 FunSpeech voice_manager 全量同步]
    B -- 否 --> D[读取本地音色注册表]
    C --> D
    D --> E[展示音色列表]
    E --> F{用户操作}
    F -->|试听| G[播放参考音频]
    F -->|切换角色| H[设置当前 voice_name]
    F -->|加载预设参数| I[写入当前参数面板]
    F -->|新增/修改/删除音色| J[更新本地音色注册表]
    G --> E
    H --> K[保存当前选中音色到本地配置]
    I --> K
    J --> L[增量同步到 FunSpeech voice_manager]
    L --> E
    K --> M[返回实时变声或离线变声页面]
```

### 1.2 时序图

```mermaid
sequenceDiagram
    actor User as 用户
    participant Desktop as 桌面端 UI
    participant Registry as 本地音色注册表
    participant VM as FunSpeech voice_manager
    participant Cache as 本地缓存

    User->>Desktop: 首次启动 / 打开音色管理
    alt 首次启动
        Desktop->>VM: 拉取全部音色
        VM-->>Desktop: 返回 voice_manager 当前音色
        Desktop->>Registry: 覆盖写入本地音色注册表
        Registry-->>Desktop: 同步完成
    else 非首次启动
        Desktop->>Registry: 读取本地音色注册表
        Registry-->>Desktop: 返回音色列表与元数据
    end
    Desktop->>Cache: 读取参考音频缓存
    Cache-->>Desktop: 返回可用试听资源
    Desktop-->>User: 展示音色列表

    User->>Desktop: 点击试听
    Desktop->>Cache: 读取 reference_audio
    Cache-->>Desktop: 返回音频
    Desktop-->>User: 播放试听

    User->>Desktop: 选择角色
    Desktop->>Registry: 读取该角色默认参数
    Registry-->>Desktop: 返回参数预设
    Desktop-->>User: 更新当前角色与参数面板

    User->>Desktop: 新增 / 修改 / 删除音色
    Desktop->>Registry: 更新本地音色注册表
    Registry-->>Desktop: 本地更新完成
    Desktop->>VM: 增量同步(add / update / delete)
    VM-->>Desktop: 同步成功
```

## 2. 实时变声

### 2.1 流程图

```mermaid
flowchart TD
    A[用户点击开始实时变声] --> B{输入设备是否可用}
    B -- 否 --> C[提示重新选择输入设备]
    B -- 是 --> D[桌面端读取当前 voice_name 与参数]
    D --> E[建立 FunSpeech Realtime Voice 会话]
    E --> F{会话是否建立成功}
    F -- 否 --> G[提示云端连接或推理异常]
    F -- 是 --> H[本地采集麦克风音频]
    H --> I[发送音频流到 FunSpeech Realtime Voice]
    I --> J[FunSpeech 执行实时变声]
    J --> K[返回连续音频块]
    K --> L[本地监听播放]
    K --> M[输出到虚拟麦克风]
    M --> N{用户是否停止}
    N -- 否 --> H
    N -- 是 --> O[关闭会话并清空缓冲]
```

### 2.2 时序图

```mermaid
sequenceDiagram
    actor User as 用户
    participant UI as 桌面端 UI
    participant Audio as 本地 Audio Engine
    participant Session as Session Manager
    participant RT as FunSpeech Realtime Voice
    participant VMic as 虚拟麦克风

    User->>UI: 点击开始实时变声
    UI->>Session: startSession(voice_name, runtime_params)
    Session->>RT: 建立实时会话
    RT-->>Session: session_ready
    Session-->>UI: 进入运行态

    loop 实时音频循环
        Audio->>Session: 采集 PCM 音频块
        Session->>RT: 发送音频块
        RT-->>Session: 返回变声音频块
        Session->>Audio: 播放监听音频
        Session->>VMic: 写入虚拟麦克风
    end

    User->>UI: 调整参数/切换角色
    UI->>Session: updateParams / updateVoice
    Session->>RT: 发送参数更新
    RT-->>Session: update_ack

    User->>UI: 停止
    UI->>Session: stopSession()
    Session->>RT: close
    Session->>Audio: 清空缓冲
    Session->>VMic: 停止输出
```

## 3. 离线变声

### 3.1 流程图

```mermaid
flowchart TD
    A[用户进入离线变声页] --> B{输入模式}
    B -- 音频文件 --> C[上传本地音频]
    B -- 文本 --> D[输入文本]
    C --> E[校验格式/时长]
    D --> F[校验文本内容]
    E --> G{校验通过?}
    F --> G
    G -- 否 --> H[提示错误并阻止提交]
    G -- 是 --> I[选择目标角色与参数]
    I --> J[提交离线任务]
    J --> K{任务类型}
    K -- 音频输入 --> L[FunSpeech ASR/TTS 或重建式输出链路]
    K -- 文本输入 --> M[FunSpeech TTS]
    L --> N[生成导出音频]
    M --> N
    N --> O[桌面端下载结果]
    O --> P[导出 WAV / MP3]
```

### 3.2 时序图

```mermaid
sequenceDiagram
    actor User as 用户
    participant UI as 桌面端 UI
    participant Job as Offline Job Manager
    participant FunSpeech as FunSpeech
    participant Store as 本地导出目录

    User->>UI: 上传音频或输入文本
    UI->>UI: 校验输入内容
    User->>UI: 选择 voice_name 与参数
    UI->>Job: createOfflineJob(input, voice_name, params)

    alt 音频输入
        Job->>FunSpeech: 提交音频处理请求
        FunSpeech-->>Job: task_id / processing
    else 文本输入
        Job->>FunSpeech: 调用 TTS(text, voice_name, params)
        FunSpeech-->>Job: task_id / processing
    end

    loop 查询状态
        Job->>FunSpeech: 查询任务状态或等待结果
        FunSpeech-->>Job: processing / done / failed
    end

    FunSpeech-->>Job: 返回导出音频
    Job->>Store: 保存导出文件
    Job-->>UI: 更新完成状态与文件路径
    UI-->>User: 提供试听与导出
```

## 4. 音色设计

### 4.1 流程图

```mermaid
flowchart TD
    A[用户进入音色设计页] --> B{输入方式}
    B -- 语音描述 --> C[录音]
    B -- 文本描述 --> D[输入文本]
    C --> E[调用 FunSpeech ASR]
    E --> F[获得 asr_text]
    D --> G[直接得到描述文本]
    F --> H[桌面端调用本地 LLM]
    G --> H
    H --> I[生成 voice_instruction / reference_text / voice_name]
    I --> J[调用 FunSpeech Voice Design 接口]
    J --> K[FunSpeech 使用 VoxCPM 生成参考音频]
    K --> L[桌面端收到 reference_audio]
    L --> M[用户试听并确认]
    M --> N{是否保存}
    N -- 否 --> O[允许重新生成或修改描述]
    N -- 是 --> P[保存到桌面端本地音色注册表]
    P --> Q[按 voice_name 增量同步到 FunSpeech voice_manager]
    Q --> R[自定义音色可在音色管理中使用]
```

### 4.2 时序图

```mermaid
sequenceDiagram
    actor User as 用户
    participant UI as 桌面端 UI
    participant Design as Voice Design Manager
    participant ASR as FunSpeech ASR
    participant LLM as 本地 LLM 服务
    participant VD as FunSpeech Voice Design
    participant Registry as 本地音色注册表
    participant VM as FunSpeech voice_manager

    User->>UI: 录音或输入文本描述

    alt 语音描述
        UI->>ASR: 发送语音描述
        ASR-->>UI: 返回 asr_text
        UI->>Design: 使用 asr_text 进入设计流程
    else 文本描述
        UI->>Design: 使用输入文本进入设计流程
    end

    Design->>LLM: generateVoiceInstruction(description)
    LLM-->>Design: voice_instruction, reference_text, voice_name
    Design->>VD: voice-design(voice_name, voice_instruction, reference_text)
    VD-->>Design: reference_audio
    Design-->>UI: 展示指令、文本、参考音频
    UI-->>User: 试听确认

    User->>UI: 点击保存
    UI->>Registry: 保存 voice_name / instruction / text / audio
    Registry-->>UI: 保存成功
    UI->>VM: register(voice_name, reference_text, reference_audio)
    VM-->>UI: 装载成功
    UI-->>User: 自定义音色可用
```

## 5. 使用建议

- 这份文档适合直接放进设计评审或实现拆解里
- 如果后续要继续细化，我建议下一步补：
  - 异常分支时序图
  - 实时变声参数更新时序图
  - 音色设计失败回退图
  - `FunSpeech` 接口契约图
