<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from 'vue';
import { useRealtimeStore } from '../stores/realtime.store';
import { logRealtimeDebug } from '../utils/realtime-debug';

const realtime = useRealtimeStore();
const emit = defineEmits<{
  openDeviceSettings: [];
}>();
let refreshTimer: number | undefined;

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
    return 'FunSpeech 已连接';
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
    return '音频帧 --';
  }
  return `发 ${snapshot.sentFrames} / 收 ${snapshot.receivedFrames}`;
});

const outputProofLabel = computed(() => {
  const snapshot = realtime.state.snapshot;
  if (!realtime.state.settings?.device.virtualMicEnabled) {
    return '虚拟麦未启用';
  }
  if (!snapshot) {
    return '虚拟麦 --';
  }
  return snapshot.virtualMicFrames > 0
    ? `虚拟麦写入 ${snapshot.virtualMicFrames}`
    : '虚拟麦等待写入';
});

const pipelineLabel = computed(() => {
  const mode = realtime.state.settings?.runtime.realtimeVoiceMode;
  const prefix = mode === 'asrTts' ? 'B ASR→TTS' : 'A Realtime Voice';
  const stage = realtime.state.snapshot?.pipelineStage;
  return stage ? `${prefix} / ${stage}` : prefix;
});

const realtimeProofHint = computed(() => {
  const snapshot = realtime.state.snapshot;
  if (!snapshot || streamState.value !== 'running') {
    return '等待实时链路建立。';
  }
  if (snapshot.receivedFrames > 0) {
    if (realtime.state.settings?.device.virtualMicEnabled && snapshot.virtualMicFrames === 0) {
      return '已收到 FunSpeech 回包，但还没有写入虚拟麦克风。';
    }
    return `已收到 FunSpeech 回包 ${snapshot.receivedFrames} 帧，可判定实时音频回路已通。`;
  }
  if (snapshot.sentFrames > 0) {
    if (realtime.state.settings?.runtime.realtimeVoiceMode === 'asrTts' && snapshot.asrText) {
      return `ASR 已识别：${snapshot.asrText}；正在等待 TTS 音频回包。`;
    }
    return `已发送麦克风音频 ${snapshot.sentFrames} 帧，正在等待 FunSpeech 回包。`;
  }
  return 'FunSpeech 已配置完成，正在等待本机麦克风输入。';
});

const callHeadline = computed(() => {
  if (realtime.state.lastError) {
    return '声音通话中断';
  }
  if (realtime.isRunning.value) {
    return '正在用当前音色通话';
  }
  if (realtime.state.busy) {
    return '正在进入语音通话';
  }
  return '准备开始语音通话';
});

const callHint = computed(() => {
  if (realtime.state.lastError) {
    return realtime.state.lastError;
  }
  if (realtime.isRunning.value) {
    return realtimeProofHint.value;
  }
  return '选择一个声音身份，点击底部按钮开始通话式实时变声。';
});

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
        <span>{{ pipelineLabel }}</span>
        <span>{{ realtime.state.snapshot?.audioMode ?? 'audio-mode --' }}</span>
      </div>
    </header>

    <div class="call-layout">
      <main class="call-room" aria-label="语音通话房间">
        <div class="call-room__glow" aria-hidden="true"></div>

        <section class="call-focus-card" :class="{ 'call-focus-card--live': realtime.isRunning.value }">
          <div class="voice-avatar-ring">
            <div class="voice-avatar">
              <span>{{ realtime.selectedVoice.value?.displayName?.slice(0, 2) ?? '声' }}</span>
            </div>
          </div>
          <p class="stage-kicker">当前声音身份</p>
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

        <nav class="call-dock" aria-label="通话控制">
          <button class="dock-button" type="button" :disabled="realtime.state.loading" @click="realtime.load">
            <span>↻</span>
            <small>刷新</small>
          </button>
          <button class="dock-button" type="button">
            <span>🎧</span>
            <small>监听</small>
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
            <small>挂断</small>
          </button>
          <button class="dock-button" type="button">
            <span>🎙</span>
            <small>麦克风</small>
          </button>
          <button class="dock-button" type="button" @click="emit('openDeviceSettings')">
            <span>⚙</span>
            <small>设置</small>
          </button>
        </nav>
      </main>
    </div>
  </section>
</template>
