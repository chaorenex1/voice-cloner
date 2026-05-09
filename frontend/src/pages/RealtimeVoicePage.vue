<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRealtimeStore } from '../stores/realtime.store';
import { logRealtimeDebug } from '../utils/realtime-debug';

const realtime = useRealtimeStore();
const emit = defineEmits<{
  openDeviceSettings: [];
}>();
let refreshTimer: number | undefined;
const voiceDrawerOpen = ref(false);
const visibleVoiceCount = ref(12);

const visibleVoices = computed(() => realtime.state.voices.slice(0, visibleVoiceCount.value));
const hasMoreVoices = computed(() => visibleVoiceCount.value < realtime.state.voices.length);

function openVoiceDrawer(): void {
  visibleVoiceCount.value = Math.min(12, Math.max(12, realtime.state.voices.length));
  voiceDrawerOpen.value = true;
}

function closeVoiceDrawer(): void {
  voiceDrawerOpen.value = false;
}

function loadMoreVoices(): void {
  visibleVoiceCount.value = Math.min(visibleVoiceCount.value + 12, realtime.state.voices.length);
}

function handleVoiceDrawerScroll(event: Event): void {
  const target = event.currentTarget as HTMLElement;
  if (target.scrollTop + target.clientHeight >= target.scrollHeight - 24 && hasMoreVoices.value) {
    loadMoreVoices();
  }
}

const meterStyle = computed(() => {
  const peak = realtime.state.snapshot?.inputLevel.peak ?? 0;
  return { width: `${Math.min(100, Math.max(4, peak * 100))}%` };
});

const streamState = computed(
  () => realtime.state.snapshot?.websocketState ?? realtime.state.session?.status ?? 'idle'
);

const statusLabel = computed(() => {
  if (realtime.state.lastError) {
    return '链路异常';
  }
  if (streamState.value === 'running') {
    return '会话已连接';
  }
  if (streamState.value === 'connecting') {
    return '连接中';
  }
  return '待机';
});

const latencyLabel = computed(() => {
  const latency = realtime.state.snapshot?.latencyMs;
  return latency === null || latency === undefined ? '延迟 --' : `延迟 ${latency}ms`;
});

const frameProofLabel = computed(() => {
  const snapshot = realtime.state.snapshot;
  if (!snapshot) {
    return 'VAD -- / 变声 --';
  }
  const received = snapshot.outputReceivedFrames || snapshot.convertedFrames || snapshot.receivedFrames;
  const written = snapshot.outputWrittenFrames || snapshot.virtualMicFrames || snapshot.monitorFrames;
  return `VAD ${snapshot.vadSpeechFrames} / 结束 ${snapshot.vadUtterancesEnded} / 收 ${received} / 写 ${written}`;
});

const outputProofLabel = computed(() => {
  const snapshot = realtime.state.snapshot;
  if (!snapshot) {
    return '监听 --';
  }
  if (snapshot.monitorState === 'listening') {
    return snapshot.monitorFrames > 0 ? `监听播放 ${snapshot.monitorFrames}` : '监听等待音频';
  }
  return '监听未开启';
});

const realtimeProofHint = computed(() => {
  const snapshot = realtime.state.snapshot;
  if (!snapshot || streamState.value !== 'running') {
    return '点击开始仅建立 FunSpeech 实时会话，不会自动打开麦克风。';
  }
  if (snapshot.backpressureHint) {
    return snapshot.backpressureHint;
  }
  if (snapshot.inputHealth) {
    return snapshot.inputHealth;
  }
  if (snapshot.lastPrompt) {
    return snapshot.lastPrompt;
  }
  const convertedFrames = snapshot.outputReceivedFrames || snapshot.convertedFrames || snapshot.receivedFrames;
  if (convertedFrames > 0) {
    if (snapshot.outputAckMismatches > 0) {
      return `已收到变声音频 ${convertedFrames} 帧，但有 ${snapshot.outputAckMismatches} 帧缺少输出元数据匹配。`;
    }
    if (snapshot.monitorState === 'listening') {
      return `已收到变声成功音频 ${convertedFrames} 帧，正在通过监听输出设备播放。`;
    }
    return `已收到变声成功音频 ${convertedFrames} 帧，可点击监听播放到输出设备。`;
  }
  if (snapshot.vadSpeechFrames > 0) {
    return `VAD 已确认并发送 ${snapshot.vadSpeechFrames} 帧，正在等待 FunSpeech 返回变声语音。`;
  }
  return '会话已就绪，点击麦克风开始采集并发送输入音频。';
});

const callHeadline = computed(() => {
  if (realtime.state.lastError) {
    return '声音通话中断';
  }
  if (realtime.isRunning.value) {
    if (!realtime.isInputCapturing.value) {
      return '实时会话已就绪';
    }
    return realtime.state.inputSource === 'localFile' ? '正在模拟本地音频输入' : '正在采集麦克风输入';
  }
  if (realtime.state.busy) {
    return '正在进入语音通话';
  }
  return '准备建立实时变声会话';
});

const callHint = computed(() => {
  if (realtime.state.lastError) {
    return realtime.state.lastError;
  }
  if (realtime.isRunning.value) {
    return realtimeProofHint.value;
  }
  return '先选择音色，点击开始建立 WebSocket；需要说话时再打开麦克风。';
});

const monitorLabel = computed(() => {
  if (realtime.state.snapshot?.monitorState === 'starting') {
    return '监听中';
  }
  return realtime.isMonitoring.value ? '停监听' : '监听';
});

const micLabel = computed(() => {
  if (realtime.state.snapshot?.inputState === 'starting') {
    return '开启中';
  }
  if (realtime.state.inputSource === 'localFile') {
    return realtime.isInputCapturing.value ? '停模拟' : '模拟';
  }
  return realtime.isInputCapturing.value ? '关麦克风' : '麦克风';
});

function handleRealtimeFileChange(event: Event): void {
  const input = event.currentTarget as HTMLInputElement;
  realtime.setSelectedInputFile(input.files?.[0] ?? null);
}

async function selectVoiceFromDrawer(voiceName: string): Promise<void> {
  await realtime.selectVoice(voiceName);
  closeVoiceDrawer();
}

onMounted(async () => {
  logRealtimeDebug('page:mounted');
  await realtime.load();
  refreshTimer = window.setInterval(() => {
    void realtime.refreshSnapshot();
  }, 700);
  logRealtimeDebug('page:refresh-timer-started', { intervalMs: 700 });
});

onBeforeUnmount(() => {
  if (refreshTimer !== undefined) {
    window.clearInterval(refreshTimer);
    logRealtimeDebug('page:refresh-timer-stopped');
  }
});
</script>

<template>
  <section class="realtime-page realtime-call-page" aria-labelledby="realtime-title">
    <header class="call-topbar">
      <div>
        <p class="module-eyebrow">Voice Call</p>
      </div>
      <div class="call-quality" aria-label="通话质量">
        <span :data-state="streamState">
          {{ statusLabel }}
        </span>
        <span>{{ latencyLabel }}</span>
        <span>{{ frameProofLabel }}</span>
        <span>{{ outputProofLabel }}</span>
        <!-- <span>{{ realtime.state.snapshot?.audioMode ?? 'audio-mode --' }}</span> -->
      </div>
    </header>

    <div class="call-layout">
      <main class="call-room" aria-label="语音通话房间">
        <div class="call-room__glow" aria-hidden="true"></div>

        <section
          class="call-focus-card"
          :class="{ 'call-focus-card--live': realtime.isRunning.value }"
        >
          <div class="voice-avatar-ring">
            <div class="voice-avatar">
              <span>{{ realtime.selectedVoice.value?.displayName?.slice(0, 2) ?? '声' }}</span>
            </div>
          </div>
          <p class="stage-kicker">实时声音身份</p>
          <h3>{{ realtime.selectedVoice.value?.displayName ?? '请选择音色' }}</h3>
          <p>{{ callHeadline }}</p>

          <div class="call-waveform" aria-label="输入电平">
            <span v-for="index in 18" :key="index" :style="{ '--bar': index }"></span>
          </div>

          <div class="call-meter">
            <span class="call-meter__fill" :style="meterStyle"></span>
          </div>
          <small class="call-hint">{{ callHint }}</small>
        </section>

        <div class="call-toast-row" aria-live="polite">
          <div class="call-bubble call-bubble--system">
            {{ realtime.state.lastError ?? realtime.state.lastMessage }}
          </div>
          <div v-if="realtime.isRunning.value" class="call-bubble call-bubble--voice">
            {{ realtimeProofHint }}
          </div>
        </div>

        <div class="input-source-panel" aria-label="实时输入源">
          <button
            type="button"
            :class="{ active: realtime.state.inputSource === 'microphone' }"
            :disabled="realtime.isInputCapturing.value"
            @click="realtime.setInputSource('microphone')"
          >
            麦克风
          </button>
          <button
            type="button"
            :class="{ active: realtime.state.inputSource === 'localFile' }"
            :disabled="realtime.isInputCapturing.value"
            @click="realtime.setInputSource('localFile')"
          >
            本地音频模拟
          </button>
          <label v-if="realtime.state.inputSource === 'localFile'" class="input-source-panel__file">
            <input
              type="file"
              accept=".wav,audio/wav,audio/x-wav"
              :disabled="realtime.isInputCapturing.value"
              @change="handleRealtimeFileChange"
            />
            <span>{{ realtime.state.selectedInputFile?.name ?? '选择 WAV 测试音频' }}</span>
          </label>
        </div>

        <nav class="call-dock" aria-label="通话控制">
          <button
            class="dock-button"
            type="button"
            :disabled="realtime.state.loading"
            @click="openVoiceDrawer"
          >
            <span>♪</span>
            <small>音色</small>
          </button>
          <button
            class="dock-button"
            type="button"
            :class="{ 'dock-button--active': realtime.isMonitoring.value }"
            :disabled="!realtime.canControlStream.value"
            @click="realtime.toggleMonitor"
          >
            <span>🎧</span>
            <small>{{ monitorLabel }}</small>
          </button>
          <button
            v-if="!realtime.isRunning.value"
            class="dock-button dock-button--primary"
            type="button"
            :disabled="!realtime.canStart.value"
            @click="realtime.start"
          >
            <span>{{ realtime.state.busy ? '…' : '▶' }}</span>
            <small>{{ realtime.state.busy ? '连接中' : '开始' }}</small>
          </button>
          <button
            v-else
            class="dock-button dock-button--hangup"
            type="button"
            :disabled="realtime.state.busy"
            @click="realtime.stop"
          >
            <span>■</span>
            <small>停止</small>
          </button>
          <button
            class="dock-button"
            type="button"
            :class="{ 'dock-button--active': realtime.isInputCapturing.value }"
            :disabled="!realtime.canControlStream.value"
            @click="realtime.toggleInput"
          >
            <span>🎙</span>
            <small>{{ micLabel }}</small>
          </button>
          <button class="dock-button" type="button" @click="emit('openDeviceSettings')">
            <span>⚙</span>
            <small>设置</small>
          </button>
        </nav>
      </main>
    </div>

    <div v-if="voiceDrawerOpen" class="voice-drawer-backdrop" @click.self="closeVoiceDrawer">
      <aside class="voice-drawer" aria-label="选择实时变声音色">
        <header class="voice-drawer__header">
          <div>
            <p class="module-eyebrow">Voice Picker</p>
            <h3>选择音色</h3>
            <small>连接中选择音色会实时发送到 FunSpeech。</small>
          </div>
          <button
            class="ghost-button"
            type="button"
            :disabled="realtime.state.loading"
            @click="realtime.load"
          >
            {{ realtime.state.loading ? '刷新中' : '刷新列表' }}
          </button>
          <button class="icon-button" type="button" @click="closeVoiceDrawer">关闭</button>
        </header>

        <div class="voice-drawer__list" @scroll="handleVoiceDrawerScroll">
          <article
            v-for="voice in visibleVoices"
            :key="voice.voiceName"
            class="voice-drawer-card"
            :class="{
              'voice-drawer-card--active': voice.voiceName === realtime.state.selectedVoiceName,
            }"
          >
            <button type="button" @click="selectVoiceFromDrawer(voice.voiceName)">
              <strong>{{ voice.displayName }}</strong>
              <span>{{
                voice.source === 'preset'
                  ? '预置音色'
                  : voice.source === 'remote'
                    ? '云端音色'
                    : '自定义音色'
              }}</span>
              <small>{{ voice.referenceTextPreview || '点击选择为实时变声目标音色' }}</small>
            </button>
          </article>
          <button
            v-if="hasMoreVoices"
            class="load-more-button"
            type="button"
            @click="loadMoreVoices"
          >
            下拉加载更多音色
          </button>
        </div>
      </aside>
    </div>
  </section>
</template>
