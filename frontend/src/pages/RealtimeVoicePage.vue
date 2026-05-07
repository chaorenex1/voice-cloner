<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from 'vue';
import { useRealtimeStore, type RealtimeParamState } from '../stores/realtime.store';

const realtime = useRealtimeStore();
let refreshTimer: number | undefined;

const statusLabel = computed(() => {
  const status = realtime.state.session?.status ?? 'idle';
  const websocket = realtime.state.snapshot?.websocketState;
  if (websocket === 'error' || realtime.state.lastError) {
    return '异常';
  }
  if (status === 'running') {
    return '运行中';
  }
  if (status === 'connecting') {
    return '连接中';
  }
  if (status === 'stopped') {
    return '已停止';
  }
  return '待机';
});

const latencyLabel = computed(() =>
  realtime.state.snapshot?.latencyMs == null ? '--' : `${realtime.state.snapshot.latencyMs}ms`
);

const inputDeviceName = computed(() => {
  const settings = realtime.state.settings;
  const devices = realtime.state.settings ? null : null;
  if (!settings?.device.inputDeviceId) {
    return '默认输入设备';
  }
  return devices ?? settings.device.inputDeviceId;
});

const outputDeviceName = computed(() => {
  const settings = realtime.state.settings;
  if (!settings?.device.outputDeviceId) {
    return '默认输出设备';
  }
  return settings.device.outputDeviceId;
});

const virtualMicLabel = computed(() => {
  const settings = realtime.state.settings;
  if (!settings?.device.virtualMicEnabled) {
    return '未启用';
  }
  if (realtime.state.snapshot?.virtualMicFrames) {
    return `输出 ${realtime.state.snapshot.virtualMicFrames} 帧`;
  }
  return settings.device.virtualMicDeviceId ? '待输出' : '未选择设备';
});

const meterStyle = computed(() => {
  const peak = realtime.state.snapshot?.inputLevel.peak ?? 0;
  return { width: `${Math.min(100, Math.max(4, peak * 100))}%` };
});

const paramRows: Array<{ key: keyof RealtimeParamState; label: string; min: number; max: number; step: number }> =
  [
    { key: 'pitch', label: '音高', min: 0.5, max: 1.5, step: 0.05 },
    { key: 'strength', label: '强度', min: 0, max: 2, step: 0.05 },
    { key: 'brightness', label: '亮度', min: 0, max: 2, step: 0.05 },
  ];

onMounted(async () => {
  await realtime.load();
  refreshTimer = window.setInterval(() => {
    void realtime.refreshSnapshot();
  }, 700);
});

onBeforeUnmount(() => {
  if (refreshTimer !== undefined) {
    window.clearInterval(refreshTimer);
  }
});
</script>

<template>
  <section class="realtime-page" aria-labelledby="realtime-title">
    <header class="realtime-header">
      <div>
        <p class="module-eyebrow">Realtime Voice</p>
        <h2 id="realtime-title">实时变声</h2>
      </div>
      <div class="realtime-header__metrics" aria-label="实时状态摘要">
        <span :data-state="realtime.state.snapshot?.websocketState ?? realtime.state.session?.status ?? 'idle'">
          FunSpeech {{ statusLabel }}
        </span>
        <span>延迟 {{ latencyLabel }}</span>
        <span>模式 {{ realtime.state.snapshot?.audioMode ?? 'passthrough' }}</span>
      </div>
    </header>

    <div class="realtime-device-strip">
      <span>输入：{{ inputDeviceName }}</span>
      <span>输出：{{ outputDeviceName }}</span>
      <span>虚拟麦克风：{{ virtualMicLabel }}</span>
      <span>WebSocket：{{ realtime.state.snapshot?.websocketUrl ?? '等待会话' }}</span>
    </div>

    <div class="realtime-workspace">
      <main class="realtime-stage" aria-label="实时控制舞台">
        <div class="voice-orb" :class="{ 'voice-orb--running': realtime.isRunning.value }">
          <span>{{ realtime.selectedVoice.value?.displayName?.slice(0, 2) ?? '声' }}</span>
        </div>

        <div class="realtime-stage__copy">
          <p class="stage-kicker">当前音色</p>
          <h3>{{ realtime.selectedVoice.value?.displayName ?? '请选择音色' }}</h3>
          <p>
            {{
              realtime.isRunning.value
                ? 'FunSpeech WebSocket 已建立，正在进行 PCM 透传闭环。'
                : '选择音色后即可启动协议联调；voice-cloner 只传 PCM，不承载变声模型。'
            }}
          </p>
        </div>

        <div class="meter-card">
          <div class="meter-card__label">
            <span>输入电平</span>
            <strong>{{ Math.round((realtime.state.snapshot?.inputLevel.peak ?? 0) * 100) }}%</strong>
          </div>
          <div class="meter-track">
            <span class="meter-fill" :style="meterStyle"></span>
          </div>
          <div class="stream-counters">
            <span>上行 {{ realtime.state.snapshot?.sentFrames ?? 0 }} 帧</span>
            <span>下行 {{ realtime.state.snapshot?.receivedFrames ?? 0 }} 帧</span>
            <span>虚拟麦 {{ realtime.state.snapshot?.virtualMicFrames ?? 0 }} 帧</span>
          </div>
        </div>

        <div class="realtime-actions">
          <button
            v-if="!realtime.isRunning.value"
            class="realtime-primary"
            type="button"
            :disabled="!realtime.canStart.value"
            @click="realtime.start"
          >
            {{ realtime.state.busy ? '连接中...' : '开始实时变声' }}
          </button>
          <button v-else class="realtime-danger" type="button" :disabled="realtime.state.busy" @click="realtime.stop">
            停止
          </button>
          <button class="ghost-button" type="button" :disabled="realtime.state.loading" @click="realtime.load">
            刷新音色 / 设置
          </button>
        </div>

        <p class="realtime-message" :class="{ 'realtime-message--error': realtime.state.lastError }">
          {{ realtime.state.lastError ?? realtime.state.lastMessage }}
        </p>
      </main>

      <aside class="realtime-sidebar" aria-label="实时控制侧栏">
        <section class="control-panel">
          <div class="panel-title">
            <span>音色</span>
            <small>{{ realtime.state.voices.length }} 个</small>
          </div>
          <div class="realtime-voice-list">
            <button
              v-for="voice in realtime.state.voices"
              :key="voice.voiceName"
              class="realtime-voice-card"
              :class="{ 'realtime-voice-card--active': voice.voiceName === realtime.state.selectedVoiceName }"
              type="button"
              :disabled="realtime.state.busy"
              @click="realtime.selectVoice(voice.voiceName)"
            >
              <strong>{{ voice.displayName }}</strong>
              <span>{{ voice.tags.join(' / ') || voice.source }}</span>
            </button>
          </div>
        </section>

        <section class="control-panel">
          <div class="panel-title">
            <span>常用参数</span>
            <small>热更新</small>
          </div>
          <label v-for="param in paramRows" :key="param.key" class="param-row">
            <span>{{ param.label }}</span>
            <input
              type="range"
              :min="param.min"
              :max="param.max"
              :step="param.step"
              :value="realtime.state.params[param.key]"
              @change="
                realtime.setParam(
                  param.key,
                  Number(($event.target as HTMLInputElement).value)
                )
              "
            />
            <strong>{{ realtime.state.params[param.key].toFixed(2) }}</strong>
          </label>
        </section>

        <section class="control-panel control-panel--compact">
          <div class="panel-title">
            <span>协议事件</span>
            <small>{{ realtime.state.snapshot?.taskId ?? '无 task' }}</small>
          </div>
          <dl class="protocol-list">
            <div>
              <dt>最后事件</dt>
              <dd>{{ realtime.state.snapshot?.lastEvent ?? '--' }}</dd>
            </div>
            <div>
              <dt>上行字节</dt>
              <dd>{{ realtime.state.snapshot?.sentBytes ?? 0 }}</dd>
            </div>
            <div>
              <dt>下行字节</dt>
              <dd>{{ realtime.state.snapshot?.receivedBytes ?? 0 }}</dd>
            </div>
          </dl>
        </section>
      </aside>
    </div>
  </section>
</template>
