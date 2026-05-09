<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { useOfflineStore } from '../stores/offline.store';
import type { OfflineJob } from '../utils/types/offline';

const offline = useOfflineStore();

const currentJob = computed(() => offline.state.currentJob);
const statusLabel = computed(() => labelForStatus(currentJob.value));

onMounted(() => {
  void offline.load();
});

function handleFileChange(event: Event): void {
  const input = event.target as HTMLInputElement;
  offline.setSelectedFile(input.files?.[0] ?? null);
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

        <label v-if="offline.state.inputType === 'audio'" class="drop-zone">
          <input type="file" accept=".wav,audio/wav,audio/x-wav" @change="handleFileChange" />
          <span>选择 WAV</span>
          <strong>{{ offline.state.selectedFile?.name ?? '拖入或点击选择一段人声音频' }}</strong>
          <small>会按 60 秒自动分块处理，全部完成后合并为一个 WAV。</small>
        </label>

        <label v-else class="text-input-block">
          <span>要生成的文本</span>
          <textarea
            v-model="offline.state.text"
            maxlength="1200"
            placeholder="输入台词、旁白或角色配音文本..."
          ></textarea>
          <small>{{ offline.state.text.trim().length }} / 1200</small>
        </label>

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
        <p class="job-message" :class="{ error: offline.state.lastError }">
          {{ offline.state.lastError ?? offline.state.lastMessage }}
        </p>

        <dl v-if="currentJob" class="job-proof-list">
          <div>
            <dt>任务 ID</dt>
            <dd>{{ currentJob.jobId }}</dd>
          </div>
          <div>
            <dt>输入</dt>
            <dd>
              {{ currentJob.inputFileName ?? (currentJob.inputType === 'text' ? '文本' : '音频') }}
            </dd>
          </div>
          <div>
            <dt>音色</dt>
            <dd>{{ currentJob.voiceName }}</dd>
          </div>
          <div>
            <dt>导出路径</dt>
            <dd>{{ currentJob.localArtifactPath ?? '尚未生成' }}</dd>
          </div>
        </dl>

        <div v-if="currentJob" class="job-actions">
          <button
            type="button"
            :disabled="!['failed', 'cancelled'].includes(currentJob.status) || offline.state.busy"
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
        </div>
      </aside>
    </div>

    <section class="offline-history" aria-labelledby="offline-history-title">
      <div class="history-heading">
        <p class="module-eyebrow">Exports</p>
        <h3 id="offline-history-title">最近离线记录</h3>
      </div>
      <div class="history-list">
        <div
          v-for="job in offline.state.jobs"
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
        </div>
        <p v-if="offline.state.jobs.length === 0" class="empty-history">暂无离线任务记录。</p>
      </div>
    </section>
  </section>
</template>

<style scoped>
.offline-page {
  min-height: 100vh;
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
  margin-top: 28px;
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
  grid-template-columns: repeat(3, 1fr);
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
}

.offline-history {
  margin-top: 24px;
  padding: 24px;
}

.history-heading h3 {
  margin: 6px 0 0;
  color: #142033;
}

.history-list {
  display: grid;
  gap: 10px;
  margin-top: 16px;
}

.history-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  align-items: center;
  width: 100%;
  padding: 14px 16px;
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.58);
  color: #182235;
  text-align: left;
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

.history-row.selected {
  background: #e6fb84;
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
  .history-row__select,
  .history-row {
    grid-template-columns: 1fr;
  }
}
</style>
