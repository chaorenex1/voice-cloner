<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue';
import VoiceDetailPanel from '../components/voice/VoiceDetailPanel.vue';
import VoiceLibraryRail from '../components/voice/VoiceLibraryRail.vue';
import VoiceLibraryToolbar from '../components/voice/VoiceLibraryToolbar.vue';
import { useVoiceLibraryStore } from '../stores/voice-library.store';
import type { WavUploadPayload } from '../services/tauri/voice-library';

const {
  state,
  filteredVoices,
  setSearch,
  loadVoices,
  selectVoice,
  updateDetail,
  attachReferenceAudio,
  recognizeReferenceAudio,
  saveSelectedVoice,
  createLocalVoice,
  removeSelectedVoice,
  previewVoice,
  runSync,
} = useVoiceLibraryStore();

const isCreating = ref(false);
const draft = reactive<{
  displayName: string;
  referenceText: string;
  upload: WavUploadPayload | null;
}>({
  displayName: '',
  referenceText: '',
  upload: null,
});

const operationTitle = computed(() => {
  switch (state.operation) {
    case 'loadingVoices':
      return '正在加载音色库';
    case 'syncingCloud':
      return '正在从云端同步';
    case 'refreshingCloud':
      return '正在刷新云端运行时';
    case 'uploadingAudio':
      return '正在读取参考音频';
    case 'recognizingAudio':
      return '正在自动识别参考文本';
    case 'savingVoice':
      return '正在保存修改';
    case 'creatingVoice':
      return '正在创建音色';
    case 'deletingVoice':
      return '正在删除音色';
    default:
      return '';
  }
});

onMounted(() => {
  if (!state.voices.length) {
    void loadVoices();
  }
});

function startCreate(): void {
  isCreating.value = true;
}

function cancelCreate(): void {
  isCreating.value = false;
}

async function submitDraft(): Promise<void> {
  if (!draft.displayName.trim() || !draft.referenceText.trim() || !draft.upload) {
    state.lastMessage = '新增音色需要名称、参考文本和 wav 参考音频';
    return;
  }

  await createLocalVoice({
    displayName: draft.displayName,
    referenceText: draft.referenceText,
    voiceInstruction: '',
    upload: draft.upload,
  });
  draft.displayName = '';
  draft.referenceText = '';
  draft.upload = null;
  isCreating.value = false;
}

async function pickWavFile(): Promise<WavUploadPayload | null> {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.wav,audio/wav,audio/x-wav';
  return new Promise((resolve) => {
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      if (!file.name.toLowerCase().endsWith('.wav')) {
        state.lastMessage = '参考音频只允许选择 wav 文件';
        resolve(null);
        return;
      }
      state.operation = 'uploadingAudio';
      try {
        const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
        resolve({ fileName: file.name, bytes });
      } finally {
        state.operation = null;
      }
    };
    input.click();
  });
}

async function chooseDraftAudio(): Promise<void> {
  draft.upload = await pickWavFile();
  if (draft.upload) {
    state.lastMessage = `已选择 wav 参考音频：${draft.upload.fileName}`;
    const text = await recognizeReferenceAudio(draft.upload);
    if (text) {
      draft.referenceText = text;
    }
  }
}

async function chooseDetailAudio(): Promise<void> {
  const upload = await pickWavFile();
  if (upload) {
    attachReferenceAudio(upload);
    const text = await recognizeReferenceAudio(upload);
    if (text) {
      updateDetail({
        referenceText: text,
        referenceTextPreview: text.slice(0, 42),
      });
    }
  }
}

function clearAudio(): void {
  updateDetail({
    hasReferenceAudio: false,
    referenceAudioFileName: undefined,
  });
  state.pendingReferenceAudio = null;
  state.lastMessage = '参考音频已从当前草稿中清除';
}

async function syncAllVoices(): Promise<void> {
  await runSync('full');
}

async function refreshCloudRuntime(): Promise<void> {
  await runSync('incremental');
}
</script>

<template>
  <section class="module-page voice-library-page">
    <VoiceLibraryToolbar
      :search="state.search"
      :loading="state.loading"
      :operation="state.operation"
      :result-count="filteredVoices.length"
      :total-count="state.voices.length"
      @update:search="setSearch"
      @create="startCreate"
      @sync="syncAllVoices"
      @refresh="refreshCloudRuntime"
    />

    <Transition name="voice-operation">
      <div v-if="state.operation" class="voice-operation-toast" role="status" aria-live="polite">
        <span class="operation-spinner" aria-hidden="true"></span>
        <span>
          <strong>{{ operationTitle }}</strong>
          <small>{{ state.lastMessage }}</small>
        </span>
      </div>
    </Transition>

    <div v-if="isCreating" class="create-voice-layout">
      <aside class="create-guide">
        <p class="module-eyebrow">新增自定义音色</p>
        <h3>采样建议</h3>
        <ul>
          <li>名称保持可读，后续会转成稳定 voiceName。</li>
          <li>参考文本建议 10-30 秒音频可完整覆盖。</li>
          <li>参考音频只允许 wav，录音尽量干净。</li>
        </ul>
      </aside>

      <form class="create-form" @submit.prevent="submitDraft">
        <div class="detail-header">
          <div>
            <p class="module-eyebrow">新建表单</p>
            <h3>新增自定义音色</h3>
          </div>
          <button class="ghost-button" type="button" @click="cancelCreate">取消</button>
        </div>

        <label class="form-field">
          <span>名称 *</span>
          <input v-model="draft.displayName" placeholder="例如：深夜播客男声" />
        </label>

        <label class="form-field">
          <span>参考文本 *</span>
          <textarea
            v-model="draft.referenceText"
            rows="7"
            placeholder="输入与参考音频一致的文本"
          ></textarea>
        </label>

        <div class="audio-panel">
          <div>
            <p class="module-eyebrow">参考音频 *</p>
            <strong>{{ draft.upload?.fileName ?? '尚未选择 wav 文件' }}</strong>
            <span>文件会保存到 ~/voice-cloner/library/custom-voices/。</span>
          </div>
          <button
            class="ghost-button"
            type="button"
            :class="{
              'button--busy':
                state.operation === 'uploadingAudio' || state.operation === 'recognizingAudio',
            }"
            :disabled="
              state.operation === 'uploadingAudio' || state.operation === 'recognizingAudio'
            "
            @click="chooseDraftAudio"
          >
            {{
              state.operation === 'recognizingAudio'
                ? '识别中'
                : state.operation === 'uploadingAudio'
                  ? '上传中'
                  : '选择 wav'
            }}
          </button>
        </div>

        <button
          class="primary-button"
          type="submit"
          :class="{ 'button--busy': state.operation === 'creatingVoice' }"
          :disabled="state.saving"
        >
          {{ state.operation === 'creatingVoice' ? '保存中' : '保存音色' }}
        </button>
      </form>
    </div>

    <div v-else class="voice-library-layout">
      <VoiceLibraryRail
        :voices="filteredVoices"
        :selected-voice-name="state.selectedVoiceName"
        :playing-voice-name="state.playingVoiceName"
        @select="selectVoice"
        @preview="previewVoice"
      />

      <VoiceDetailPanel
        :detail="state.detail"
        :saving="state.saving"
        :playing="state.playingVoiceName === state.detail?.voiceName"
        :operation="state.operation"
        @update-detail="updateDetail"
        @preview="previewVoice"
        @save="saveSelectedVoice"
        @delete="removeSelectedVoice"
        @upload-audio="chooseDetailAudio"
        @clear-audio="clearAudio"
      />
    </div>

    <!-- <footer class="page-status-bar">
      <span>{{ state.lastMessage }}</span>
    </footer> -->
  </section>
</template>
