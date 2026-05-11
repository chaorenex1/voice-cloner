<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { useVoiceSeparationStore } from '../stores/voice-separation.store';
import {
  lufsPresetOptions,
  type VoiceSeparationJob,
  type VoiceSeparationStemName,
} from '../utils/types/voice-separation';

const separation = useVoiceSeparationStore();

const runtimeReady = computed(
  () => separation.state.runtime?.ffmpegAvailable && separation.state.runtime?.demucsRsAvailable
);
const running = computed(() => separation.state.jobs.find((job) => !isTerminal(job)) ?? null);
const stemRows: Array<{
  key: VoiceSeparationStemName;
  label: string;
  fileName: string;
  saveable: boolean;
}> = [
  { key: 'vocals', label: '人声', fileName: 'vocals.wav', saveable: true },
  { key: 'noVocals', label: '伴奏', fileName: 'no_vocals.wav', saveable: false },
  { key: 'drums', label: '鼓', fileName: 'drums.wav', saveable: false },
  { key: 'bass', label: '贝斯', fileName: 'bass.wav', saveable: false },
  { key: 'other', label: '其他', fileName: 'other.wav', saveable: false },
];

onMounted(() => {
  void separation.load();
});

function isTerminal(job: VoiceSeparationJob): boolean {
  return ['ready', 'saved', 'failed', 'cancelled'].includes(job.status);
}

function stemPath(job: VoiceSeparationJob, stem: VoiceSeparationStemName): string | null {
  return job.stems?.[stem] ?? null;
}

function statusLabel(job: VoiceSeparationJob): string {
  switch (job.status) {
    case 'ready':
      return '已完成';
    case 'saved':
      return '已入库';
    case 'failed':
      return '失败';
    case 'cancelled':
      return '已取消';
    default:
      return `${Math.round(job.progress * 100)}%`;
  }
}
</script>

<template>
  <section class="module-page voice-separation-page voice-separation-page--fixed">
    <div class="separation-hero">
      <div>
        <p class="module-eyebrow">人声分离</p>
        <p class="module-description">
          统一选择源材料，后台自动识别音频/视频；本地 demucs-rs 分离，ffmpeg 完成伴奏合成与后处理。
        </p>
      </div>
      <button
        class="ghost-button"
        type="button"
        :disabled="separation.state.loading"
        @click="separation.load"
      >
        刷新运行时
      </button>
    </div>

    <div
      v-if="separation.state.runtime?.warnings.length"
      class="settings-warning separation-runtime-warning"
    >
      <strong>{{ runtimeReady ? '运行时提示' : '运行时不可用' }}</strong>
      <p v-for="warning in separation.state.runtime.warnings" :key="warning">{{ warning }}</p>
    </div>

    <section class="separation-command-card">
      <div class="separation-card-section">
        <div class="separation-section-heading">
          <span class="module-eyebrow">源材料选择</span>
          <p>选择一个音频或视频文件，后台会自动识别类型并准备本地分离。</p>
        </div>
        <button class="source-picker-span" type="button" @click="separation.chooseSource">
          <strong>{{ separation.state.sourceFileName ?? '选择视频或音频文件' }}</strong>
          <small>{{
            separation.state.sourcePath ?? '支持 mp4 / mov / wav / mp3 / flac 等源材料'
          }}</small>
        </button>
      </div>

      <div class="separation-card-section">
        <div class="separation-section-heading">
          <span class="module-eyebrow">参数选择</span>
          <!-- <p>控制 Demucs 模型与 ffmpeg 后处理，响度预设可按 ASR、短视频或音乐人声选择。</p> -->
        </div>
        <div class="separation-parameter-grid">
          <label class="compact-field">
            <span>模型</span>
            <select
              :value="separation.state.model"
              @change="separation.setModel(($event.target as HTMLSelectElement).value as any)"
            >
              <option value="htDemucs">HTDemucs</option>
              <option value="htDemucsFt">HTDemucs FT</option>
            </select>
          </label>

          <label class="compact-field">
            <span>降噪</span>
            <select
              :value="separation.state.postProcessConfig.denoiseMode"
              @change="
                separation.updatePostProcessConfig({
                  denoiseMode: ($event.target as HTMLSelectElement).value as any,
                })
              "
            >
              <option value="off">关闭</option>
              <option value="standard">标准</option>
              <option value="strong">强</option>
            </select>
          </label>

          <label class="compact-field compact-field--short">
            <span>采样率</span>
            <select
              :value="separation.state.postProcessConfig.targetSampleRate"
              @change="
                separation.updatePostProcessConfig({
                  targetSampleRate: Number(($event.target as HTMLSelectElement).value),
                })
              "
            >
              <option :value="24000">24k</option>
              <option :value="48000">48k</option>
            </select>
          </label>

          <label class="compact-field compact-field--lufs">
            <span>响度</span>
            <select
              :value="separation.state.postProcessConfig.targetLufs"
              @change="
                separation.updatePostProcessConfig({
                  targetLufs: Number(($event.target as HTMLSelectElement).value),
                  loudnessNormalization: true,
                })
              "
            >
              <option v-for="option in lufsPresetOptions" :key="option.value" :value="option.value">
                {{ option.label }}
              </option>
            </select>
          </label>
        </div>
      </div>

      <div class="separation-card-section separation-card-section--action">
        <button
          class="primary-button separation-start-button"
          type="button"
          :class="{ 'button--busy': !!running }"
          :disabled="!separation.canStart.value || !!running"
          @click="separation.start"
        >
          {{ separation.startButtonText.value }}
        </button>
      </div>
    </section>

    <section class="separation-results-panel">
      <div class="result-panel-header">
        <div>
          <p class="module-eyebrow">结果试听</p>
          <h3>任务结果</h3>
        </div>
        <button
          class="ghost-button"
          type="button"
          :disabled="!running"
          @click="separation.cancelCurrent"
        >
          取消当前任务
        </button>
      </div>

      <div class="result-tree-scroll">
        <article
          v-for="job in separation.visibleJobs.value"
          :key="job.jobId"
          class="result-task-node"
        >
          <button
            class="result-task-header"
            type="button"
            @click="separation.toggleJobExpanded(job.jobId)"
          >
            <span class="tree-caret">{{ separation.isJobExpanded(job.jobId) ? '▾' : '▸' }}</span>
            <span class="task-main">
              <strong>{{ job.sourceFileName }}</strong>
              <small>{{ job.currentStageMessage }}</small>
            </span>
            <span class="task-meta">{{ job.sourceType === 'video' ? '视频' : '音频' }}</span>
            <span class="task-status" :class="`task-status--${job.status}`">{{
              statusLabel(job)
            }}</span>
          </button>

          <div v-if="separation.isJobExpanded(job.jobId)" class="result-stem-children">
            <div v-if="job.errorMessage" class="settings-warning">
              <strong>任务失败</strong>
              <p>{{ job.errorMessage }}</p>
            </div>

            <div
              v-for="stem in stemRows"
              :key="stem.key"
              class="result-stem-row"
              :class="{ 'result-stem-row--missing': !stemPath(job, stem.key) }"
            >
              <span class="stem-file">
                <strong>{{ stem.label }}</strong>
                <small>{{ stem.fileName }}</small>
              </span>
              <span class="stem-path">{{ stemPath(job, stem.key) ?? '等待生成' }}</span>
              <span class="stem-actions-inline">
                <button
                  class="ghost-button"
                  type="button"
                  :disabled="!stemPath(job, stem.key) || separation.state.busy"
                  @click="separation.togglePreview(job, stem.key)"
                >
                  {{
                    separation.state.playingJobId === job.jobId &&
                    separation.state.playingStem === stem.key
                      ? '停止'
                      : '试听'
                  }}
                </button>
                <button
                  class="ghost-button"
                  type="button"
                  :disabled="!stemPath(job, stem.key) || separation.state.busy"
                  @click="separation.downloadStem(job, stem.key)"
                >
                  下载
                </button>
                <button
                  v-if="stem.saveable"
                  class="primary-button stem-save-button"
                  type="button"
                  :disabled="job.status !== 'ready' || separation.state.busy"
                  @click="separation.openSaveVoiceDialog(job)"
                >
                  保存为音色
                </button>
              </span>
            </div>
          </div>
        </article>

        <button
          v-if="separation.hasMoreJobs.value"
          class="load-more-results"
          type="button"
          @click="separation.loadMoreResults"
        >
          加载更多任务
        </button>
      </div>
    </section>

    <div v-if="separation.state.saveDialog.open" class="modal-backdrop" role="presentation">
      <form class="save-voice-dialog" @submit.prevent="separation.confirmSaveVoice">
        <p class="module-eyebrow">保存人声为自定义音色</p>
        <h3>输入音色名称</h3>
        <p>确认后会自动识别人声参考文本，并将 vocals.wav 保存到自定义音色库。</p>
        <label class="form-field">
          <span>音色名称 *</span>
          <input
            v-model="separation.state.saveDialog.voiceName"
            autofocus
            placeholder="例如：素材人声 A"
          />
        </label>
        <div class="dialog-actions">
          <button
            class="ghost-button"
            type="button"
            :disabled="separation.state.saveDialog.busy"
            @click="separation.closeSaveVoiceDialog"
          >
            取消
          </button>
          <button
            class="primary-button"
            type="submit"
            :disabled="
              !separation.state.saveDialog.voiceName.trim() || separation.state.saveDialog.busy
            "
          >
            {{ separation.state.saveDialog.busy ? '保存中' : '确认保存' }}
          </button>
        </div>
      </form>
    </div>

    <div v-if="separation.state.lastError" class="settings-warning separation-error">
      <strong>错误</strong>
      <p>{{ separation.state.lastError }}</p>
    </div>
  </section>
</template>
