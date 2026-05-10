<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import { useOfflineStore } from '../stores/offline.store';
import type { OfflineJob } from '../utils/types/offline';

const offline = useOfflineStore();
const INITIAL_HISTORY_COUNT = 8;
const HISTORY_BATCH_SIZE = 8;
const visibleHistoryCount = ref(INITIAL_HISTORY_COUNT);

const currentJob = computed(() => offline.state.currentJob);
const statusLabel = computed(() => labelForStatus(currentJob.value));
const isCurrentJobRunning = computed(() => currentJob.value?.status === 'running');
const progressStyle = computed(() => ({
  width: `${Math.min(Math.max(currentJob.value?.progress ?? 0, 0), 100)}%`,
}));
const visibleJobs = computed(() => offline.state.jobs.slice(0, visibleHistoryCount.value));
const hasMoreJobs = computed(() => visibleHistoryCount.value < offline.state.jobs.length);

onMounted(() => {
  void offline.load();
});

watch(
  () => offline.state.jobs.length,
  (length, previousLength) => {
    if (length === 0) {
      visibleHistoryCount.value = INITIAL_HISTORY_COUNT;
      return;
    }
    if (length < previousLength) {
      visibleHistoryCount.value = Math.max(
        INITIAL_HISTORY_COUNT,
        Math.min(visibleHistoryCount.value, length)
      );
    }
  }
);

function handleFileChange(event: Event): void {
  const input = event.target as HTMLInputElement;
  offline.setSelectedFile(input.files?.[0] ?? null);
}

function loadMoreHistory(): void {
  visibleHistoryCount.value = Math.min(
    visibleHistoryCount.value + HISTORY_BATCH_SIZE,
    offline.state.jobs.length
  );
}

function handleHistoryScroll(event: Event): void {
  const target = event.currentTarget as HTMLElement;
  if (target.scrollTop + target.clientHeight >= target.scrollHeight - 24 && hasMoreJobs.value) {
    loadMoreHistory();
  }
}

function labelForStatus(job: OfflineJob | null): string {
  if (!job) {
    return '未创建任务';
  }
  switch (job.status) {
    case 'created':
      return '等待开始';
    case 'running':
      return `处理中 · ${stageLabel(job.stage)}`;
    case 'completed':
      return '已完成';
    case 'failed':
      return '失败';
    case 'cancelled':
      return '已取消';
  }
}

function stageLabel(stage: string): string {
  if (stage.startsWith('transcribingChunk:')) {
    return `识别分块 ${stage.replace('transcribingChunk:', '')}`;
  }
  if (stage.startsWith('synthesizingChunk:')) {
    return `生成分块 ${stage.replace('synthesizingChunk:', '')}`;
  }
  const labels: Record<string, string> = {
    created: '已创建',
    preparing: '准备中',
    splittingAudio: '按 60 秒切分音频',
    transcribing: '识别音频',
    synthesizing: '生成目标音色',
    mergingChunks: '合并分块音频',
    writingArtifact: '写入导出文件',
    completed: '导出完成',
    failed: '失败',
    cancelled: '已取消',
  };
  return labels[stage] ?? stage;
}
</script>

<template>
  <section class="offline-page" aria-labelledby="offline-title">
    <header class="offline-hero">
      <div>
        <p class="module-eyebrow">Offline Voice</p>
        <p>
          上传单轨人声或输入文本，选择目标音色后生成可导出的音频文件。音频输入当前走 ASR → TTS
          重建式输出。
        </p>
      </div>
      <!-- <div class="offline-status-card">
        <span>{{ statusLabel }}</span>
        <strong>{{ currentJob?.progress ?? 0 }}%</strong>
        <div class="offline-progress" aria-hidden="true">
          <i :style="progressStyle"></i>
        </div>
      </div> -->
    </header>

    <div class="offline-grid">
      <main class="offline-card offline-card--input">
        <div class="segmented-control" aria-label="输入模式">
          <button
            type="button"
            :class="{ active: offline.state.inputType === 'audio' }"
            @click="offline.setInputType('audio')"
          >
            音频文件
          </button>
          <button
            type="button"
            :class="{ active: offline.state.inputType === 'text' }"
            @click="offline.setInputType('text')"
          >
            文本输入
          </button>
        </div>

        <Transition name="offline-panel" mode="out-in">
          <label v-if="offline.state.inputType === 'audio'" key="audio" class="drop-zone">
            <input type="file" accept=".wav,audio/wav,audio/x-wav" @change="handleFileChange" />
            <span>选择 WAV</span>
            <strong>{{ offline.selectedAudioLabel.value }}</strong>
            <small>10 秒内直接识别；超过 10 秒会按 60 秒自动分块异步识别。</small>
            <button class="drop-zone__picker" type="button" @click.prevent="offline.chooseAudioFile">
              从本机选择音频
            </button>
          </label>

          <label v-else key="text" class="text-input-block">
            <span>要生成的文本</span>
            <textarea
              v-model="offline.state.text"
              maxlength="1200"
              placeholder="输入台词、旁白或角色配音文本..."
            ></textarea>
            <small>{{ offline.state.text.trim().length }} / 1200</small>
          </label>
        </Transition>

        <div class="offline-form-row">
          <label>
            目标音色
            <select v-model="offline.state.selectedVoiceName">
              <option
                v-for="voice in offline.state.voices"
                :key="voice.voiceName"
                :value="voice.voiceName"
              >
                {{ voice.displayName || voice.voiceName }}
              </option>
            </select>
          </label>
          <label>
            导出格式
            <select v-model="offline.state.outputFormat">
              <option value="wav">WAV</option>
            </select>
          </label>
        </div>

        <div class="offline-param-grid">
          <label>
            音调
            <input
              v-model.number="offline.state.params.pitchRate"
              type="range"
              min="-500"
              max="500"
              step="10"
            />
            <span>{{ offline.state.params.pitchRate }}</span>
          </label>
          <label>
            语速
            <input
              v-model.number="offline.state.params.speechRate"
              type="range"
              min="-500"
              max="500"
              step="10"
            />
            <span>{{ offline.state.params.speechRate }}</span>
          </label>
          <label>
            音量
            <input
              v-model.number="offline.state.params.volume"
              type="range"
              min="0"
              max="100"
              step="1"
            />
            <span>{{ offline.state.params.volume }}</span>
          </label>
          <label class="emotion-select">
            情感指令
            <select v-model="offline.state.selectedEmotionLabel">
              <option :value="null">不使用情感指令</option>
              <option
                v-for="emotion in offline.state.emotionOptions"
                :key="emotion.id"
                :value="emotion.label"
              >
                {{ emotion.label }}
              </option>
            </select>
            <span>{{ offline.selectedEmotion.value?.label ?? '默认' }}</span>
          </label>
        </div>

        <button
          class="offline-submit"
          type="button"
          :disabled="!offline.canSubmit.value"
          @click="offline.submit"
        >
          {{ offline.state.busy ? '处理中...' : '开始离线转换' }}
        </button>
      </main>

      <aside class="offline-card offline-card--result" aria-live="polite">
        <p class="panel-kicker">Current Job</p>
        <h3>{{ currentJob ? statusLabel : '等待任务' }}</h3>
        <Transition name="offline-panel" mode="out-in">
          <div v-if="currentJob && isCurrentJobRunning" class="job-loader" aria-label="任务处理中">
            <div class="job-loader__orb" aria-hidden="true">
              <span></span>
              <span></span>
            </div>
            <div class="job-loader__copy">
              <strong>{{ currentJob.progress }}%</strong>
              <small>{{ stageLabel(currentJob.stage) }}</small>
              <div class="job-loader__track" aria-hidden="true">
                <i :style="progressStyle"></i>
              </div>
            </div>
          </div>
        </Transition>
        <Transition name="offline-panel" mode="out-in">
          <p
            :key="offline.state.lastError ?? offline.state.lastMessage"
            class="job-message"
            :class="{ error: offline.state.lastError }"
          >
            {{ offline.state.lastError ?? offline.state.lastMessage }}
          </p>
        </Transition>

        <Transition name="offline-panel" mode="out-in">
          <dl
            v-if="currentJob"
            :key="`${currentJob.jobId}-${currentJob.stage}`"
            class="job-proof-list"
          >
            <div>
              <dt>任务 ID</dt>
              <dd>{{ currentJob.jobId }}</dd>
            </div>
            <div>
              <dt>输入</dt>
              <dd>
                {{
                  currentJob.inputFileName ?? (currentJob.inputType === 'text' ? '文本' : '音频')
                }}
              </dd>
            </div>
            <div>
              <dt>音色</dt>
              <dd>{{ currentJob.voiceName }}</dd>
            </div>
          </dl>
        </Transition>

        <Transition name="offline-panel" mode="out-in">
          <div
            v-if="currentJob"
            :key="`${currentJob.jobId}-${currentJob.status}`"
            class="job-actions"
          >
            <button
              type="button"
              :disabled="
                !['failed', 'cancelled', 'completed'].includes(currentJob.status) ||
                offline.state.busy
              "
              @click="offline.retry(currentJob)"
            >
              重试
            </button>
            <button
              type="button"
              :disabled="currentJob.status !== 'running' || offline.state.busy"
              @click="offline.cancel(currentJob)"
            >
              取消
            </button>
            <button
              type="button"
              :disabled="!offline.canPreviewJob(currentJob) || offline.state.busy"
              @click="offline.togglePreview(currentJob)"
            >
              {{ offline.state.playingJobId === currentJob.jobId ? '停止试听' : '试听' }}
            </button>
            <button
              type="button"
              :disabled="!offline.canDownloadJob(currentJob) || offline.state.busy"
              @click="offline.download(currentJob)"
            >
              下载
            </button>
          </div>
        </Transition>
      </aside>
    </div>

    <section class="offline-history" aria-labelledby="offline-history-title">
      <div class="history-heading">
        <div>
          <p class="module-eyebrow">Exports</p>
          <h3 id="offline-history-title">最近离线记录</h3>
        </div>
        <button
          class="history-clear"
          type="button"
          :disabled="offline.state.jobs.length === 0 || offline.state.busy"
          @click="offline.clearHistory"
        >
          清理记录和音频
        </button>
      </div>
      <TransitionGroup name="offline-list" tag="div" class="history-list" @scroll="handleHistoryScroll">
        <div
          v-for="job in visibleJobs"
          :key="job.jobId"
          class="history-row"
          :class="{ selected: job.jobId === currentJob?.jobId }"
        >
          <button class="history-row__select" type="button" @click="offline.selectJob(job)">
            <span>{{ job.inputType === 'audio' ? '音频' : '文本' }}</span>
            <strong>{{ job.inputFileName ?? job.inputRef.slice(0, 32) }}</strong>
            <small>{{ labelForStatus(job) }} · {{ job.outputFormat.toUpperCase() }}</small>
          </button>
          <button
            class="history-row__preview"
            type="button"
            :disabled="!offline.canPreviewJob(job) || offline.state.busy"
            @click="offline.togglePreview(job)"
          >
            {{ offline.state.playingJobId === job.jobId ? '停止' : '试听' }}
          </button>
          <button
            class="history-row__download"
            type="button"
            :disabled="!offline.canDownloadJob(job) || offline.state.busy"
            @click="offline.download(job)"
          >
            下载
          </button>
          <button
            class="history-row__delete"
            type="button"
            :disabled="offline.state.busy"
            @click="offline.deleteJob(job)"
          >
            删除
          </button>
        </div>
      </TransitionGroup>
      <button v-if="hasMoreJobs" class="history-load-more" type="button" @click="loadMoreHistory">
        下拉加载更多记录
      </button>
      <p v-if="offline.state.jobs.length === 0" class="empty-history">暂无离线任务记录。</p>
    </section>
  </section>
</template>

<style scoped>
.offline-page {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  gap: 24px;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  padding: clamp(28px, 4vw, 54px);
  background:
    radial-gradient(circle at 18% 16%, rgba(230, 251, 132, 0.35), transparent 26%),
    radial-gradient(circle at 80% 10%, rgba(82, 123, 255, 0.16), transparent 28%),
    linear-gradient(135deg, #fbf7ea 0%, #eef4f1 56%, #e4edf6 100%);
}

.offline-hero {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 260px;
  gap: 24px;
  align-items: end;
}

.offline-hero h2 {
  margin: 8px 0 12px;
  color: #101c29;
  font-size: clamp(2.6rem, 6vw, 5.4rem);
  line-height: 0.92;
  letter-spacing: -0.07em;
}

.offline-hero p {
  max-width: 720px;
  margin: 0;
  color: #657384;
}

.offline-status-card,
.offline-card,
.offline-history {
  border: 1px solid rgba(20, 32, 46, 0.12);
  border-radius: 30px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 24px 70px rgba(18, 30, 44, 0.1);
  backdrop-filter: blur(16px);
}

.offline-status-card {
  padding: 22px;
}

.offline-status-card span,
.panel-kicker {
  color: #71800f;
  font-size: 0.72rem;
  font-weight: 840;
  letter-spacing: 0.13em;
  text-transform: uppercase;
}

.offline-status-card strong {
  display: block;
  margin-top: 10px;
  color: #101c29;
  font-size: 2.6rem;
  line-height: 1;
}

.offline-progress {
  height: 10px;
  margin-top: 16px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(16, 28, 41, 0.1);
}

.offline-progress i {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #d8f95d, #5aa9ff);
}

.offline-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(300px, 0.75fr);
  gap: 24px;
  min-height: 0;
}

.offline-card {
  padding: clamp(22px, 3vw, 32px);
}

.segmented-control {
  display: inline-grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 6px;
  padding: 6px;
  border-radius: 999px;
  background: rgba(16, 28, 41, 0.08);
}

.segmented-control button,
.job-actions button,
.history-clear,
.history-row__delete,
.history-row__download,
.history-row__preview,
.offline-submit {
  border-radius: 999px;
  cursor: pointer;
  font-weight: 800;
}

.segmented-control button {
  padding: 10px 16px;
  background: transparent;
  color: #526171;
}

.segmented-control button.active {
  background: #10202f;
  color: #f7fbff;
}

.drop-zone,
.text-input-block {
  display: grid;
  gap: 10px;
  margin-top: 24px;
  padding: 26px;
  border: 1px dashed rgba(16, 28, 41, 0.26);
  border-radius: 26px;
  background:
    repeating-linear-gradient(-45deg, rgba(16, 28, 41, 0.035) 0 1px, transparent 1px 16px),
    rgba(255, 255, 255, 0.48);
}

.drop-zone input {
  display: none;
}

.drop-zone__picker {
  width: fit-content;
  margin-top: 6px;
  padding: 9px 14px;
  border-radius: 999px;
  background: #10202f;
  color: #f7fbff;
  cursor: pointer;
  font-weight: 800;
}

.drop-zone span,
.text-input-block span,
.offline-form-row label,
.offline-param-grid label {
  color: #71800f;
  font-size: 0.78rem;
  font-weight: 820;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.drop-zone strong {
  color: #122033;
  font-size: 1.35rem;
}

.drop-zone small,
.text-input-block small {
  color: #6f7c8a;
}

.text-input-block textarea {
  min-height: 180px;
  resize: vertical;
  border: 0;
  outline: 0;
  background: transparent;
  color: #152335;
  font-size: 1.02rem;
}

.offline-form-row {
  display: grid;
  grid-template-columns: 1fr 160px;
  gap: 16px;
  margin-top: 18px;
}

.offline-form-row label,
.offline-param-grid label {
  display: grid;
  gap: 8px;
}

select {
  width: 100%;
  border: 1px solid rgba(16, 28, 41, 0.14);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.74);
  color: #152335;
  padding: 12px 14px;
}

.offline-param-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
  margin-top: 18px;
}

.offline-param-grid input {
  accent-color: #10202f;
}

.offline-param-grid span {
  color: #152335;
  font-size: 0.9rem;
  font-weight: 800;
}

.offline-submit {
  width: 100%;
  margin-top: 24px;
  padding: 16px 20px;
  background: #10202f;
  color: #f7fbff;
  box-shadow: 0 16px 38px rgba(16, 32, 47, 0.22);
}

.offline-submit:disabled,
.job-actions button:disabled,
.history-clear:disabled,
.history-row__delete:disabled,
.history-row__download:disabled,
.history-row__preview:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.offline-card--result h3 {
  margin: 10px 0 8px;
  color: #142033;
  font-size: 1.8rem;
}

.job-message {
  min-height: 48px;
  color: #657384;
}

.job-message.error {
  color: #ad352e;
}

.job-loader {
  display: grid;
  grid-template-columns: 54px minmax(0, 1fr);
  gap: 14px;
  align-items: center;
  margin: 18px 0 6px;
  padding: 14px;
  border: 1px solid rgba(16, 32, 47, 0.08);
  border-radius: 22px;
  background:
    linear-gradient(120deg, rgba(230, 251, 132, 0.58), rgba(255, 255, 255, 0.42)),
    radial-gradient(circle at 92% 12%, rgba(90, 169, 255, 0.24), transparent 34%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.58);
}

.job-loader__orb {
  position: relative;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  background: #10202f;
  box-shadow:
    0 12px 28px rgba(16, 32, 47, 0.28),
    inset 0 0 0 10px rgba(230, 251, 132, 0.18);
}

.job-loader__orb::before {
  position: absolute;
  inset: 13px;
  border-radius: inherit;
  background: #e6fb84;
  content: '';
  animation: job-pulse 1.1s ease-in-out infinite;
}

.job-loader__orb span {
  position: absolute;
  inset: -5px;
  border: 2px solid transparent;
  border-top-color: #10202f;
  border-right-color: rgba(16, 32, 47, 0.25);
  border-radius: inherit;
  animation: job-spin 1.05s linear infinite;
}

.job-loader__orb span + span {
  inset: 5px;
  border-top-color: #5aa9ff;
  animation-duration: 1.45s;
  animation-direction: reverse;
}

.job-loader__copy {
  display: grid;
  gap: 6px;
  min-width: 0;
}

.job-loader__copy strong {
  color: #101c29;
  font-size: 1.35rem;
  line-height: 1;
}

.job-loader__copy small {
  overflow: hidden;
  color: #657384;
  font-weight: 760;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.job-loader__track {
  height: 9px;
  overflow: hidden;
  border-radius: 999px;
  background: rgba(16, 32, 47, 0.1);
}

.job-loader__track i {
  display: block;
  height: 100%;
  border-radius: inherit;
  background:
    linear-gradient(90deg, #10202f 0%, #5aa9ff 52%, #e6fb84 100%),
    repeating-linear-gradient(45deg, transparent 0 8px, rgba(255, 255, 255, 0.45) 8px 16px);
  box-shadow: 0 0 18px rgba(90, 169, 255, 0.36);
  transition: width 260ms ease;
}

.job-proof-list {
  display: grid;
  gap: 12px;
  margin: 20px 0 0;
}

.job-proof-list div {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.job-proof-list dt {
  color: #71800f;
  font-size: 0.72rem;
  font-weight: 800;
  text-transform: uppercase;
}

.job-proof-list dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  color: #182235;
}

.job-actions {
  display: flex;
  gap: 10px;
  margin-top: 22px;
}

.job-actions button {
  padding: 10px 16px;
  background: rgba(16, 32, 47, 0.08);
  color: #10202f;
  transition:
    transform 180ms ease,
    background 180ms ease,
    opacity 180ms ease;
}

.job-actions button:hover:not(:disabled) {
  transform: translateY(-2px);
  background: rgba(16, 32, 47, 0.14);
}

.offline-history {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  min-height: 0;
  overflow: hidden;
  padding: 24px;
}

.history-heading {
  display: flex;
  gap: 16px;
  align-items: center;
  justify-content: space-between;
}

.history-heading h3 {
  margin: 6px 0 0;
  color: #142033;
}

.history-clear {
  padding: 10px 14px;
  background: rgba(173, 53, 46, 0.1);
  color: #8f2b25;
  transition:
    transform 180ms ease,
    background 180ms ease,
    opacity 180ms ease;
}

.history-list {
  display: grid;
  gap: 10px;
  min-height: 0;
  margin-top: 16px;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding-right: 6px;
  scrollbar-gutter: stable;
}

.history-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto auto;
  gap: 12px;
  align-items: center;
  width: 100%;
  padding: 14px 16px;
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.58);
  color: #182235;
  text-align: left;
  transition:
    transform 200ms ease,
    background 200ms ease,
    box-shadow 200ms ease,
    opacity 200ms ease;
}

.history-row__select {
  display: grid;
  grid-template-columns: 70px minmax(0, 1fr) 180px;
  gap: 12px;
  align-items: center;
  min-width: 0;
  color: inherit;
  text-align: left;
}

.history-row__preview {
  padding: 8px 14px;
  background: #10202f;
  color: #f7fbff;
  font-weight: 800;
}

.history-row__download {
  padding: 8px 14px;
  background: rgba(16, 32, 47, 0.08);
  color: #10202f;
}

.history-row__delete {
  padding: 8px 14px;
  background: rgba(173, 53, 46, 0.1);
  color: #8f2b25;
}

.history-clear:hover:not(:disabled),
.history-row__delete:hover:not(:disabled),
.history-row__preview:hover:not(:disabled),
.history-row__download:hover:not(:disabled),
.offline-submit:hover:not(:disabled) {
  transform: translateY(-2px);
}

.history-row.selected {
  background: #e6fb84;
  box-shadow: 0 16px 34px rgba(113, 128, 15, 0.18);
}

.history-load-more {
  width: fit-content;
  margin: 12px auto 0;
  padding: 9px 14px;
  border-radius: 999px;
  background: rgba(16, 32, 47, 0.08);
  color: #10202f;
  cursor: pointer;
  font-weight: 800;
}

.offline-list-enter-active,
.offline-list-leave-active {
  transition:
    opacity 220ms ease,
    transform 220ms ease;
}

.offline-list-enter-from,
.offline-list-leave-to {
  opacity: 0;
  transform: translateY(10px) scale(0.98);
}

.offline-list-move {
  transition: transform 220ms ease;
}

.offline-panel-enter-active,
.offline-panel-leave-active {
  transition:
    opacity 180ms ease,
    transform 180ms ease,
    filter 180ms ease;
}

.offline-panel-enter-from,
.offline-panel-leave-to {
  opacity: 0;
  filter: blur(3px);
  transform: translateY(8px);
}

@keyframes job-spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes job-pulse {
  0%,
  100% {
    transform: scale(0.76);
    opacity: 0.82;
  }

  50% {
    transform: scale(1);
    opacity: 1;
  }
}

.history-row__select strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-row__select span,
.history-row__select small,
.empty-history {
  color: #657384;
}

.empty-history {
  margin: 0;
}

@media (max-width: 980px) {
  .offline-hero,
  .offline-grid,
  .offline-form-row,
  .offline-param-grid,
  .history-heading,
  .history-row__select,
  .history-row {
    grid-template-columns: 1fr;
  }

  .history-heading {
    align-items: stretch;
  }
}
</style>
