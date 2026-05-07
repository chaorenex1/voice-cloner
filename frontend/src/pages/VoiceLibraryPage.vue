<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue';
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
  saveSelectedVoice,
  setCurrentVoice,
  createLocalVoice,
  removeSelectedVoice,
  previewVoice,
  runSync,
} = useVoiceLibraryStore();

const isCreating = ref(false);
const draft = reactive<{
  displayName: string;
  referenceText: string;
  voiceInstruction: string;
  upload: WavUploadPayload | null;
}>({
  displayName: '',
  referenceText: '',
  voiceInstruction: '',
  upload: null,
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
    voiceInstruction: draft.voiceInstruction,
    upload: draft.upload,
  });
  draft.displayName = '';
  draft.referenceText = '';
  draft.voiceInstruction = '';
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
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      resolve({ fileName: file.name, bytes });
    };
    input.click();
  });
}

async function chooseDraftAudio(): Promise<void> {
  draft.upload = await pickWavFile();
  if (draft.upload) {
    state.lastMessage = `已选择 wav 参考音频：${draft.upload.fileName}`;
  }
}

async function chooseDetailAudio(): Promise<void> {
  const upload = await pickWavFile();
  if (upload) {
    attachReferenceAudio(upload);
  }
}

function clearAudio(): void {
  updateDetail({
    hasReferenceAudio: false,
    referenceAudioPath: undefined,
    referenceAudioFileName: undefined,
  });
  state.pendingReferenceAudio = null;
  state.lastMessage = '参考音频已从当前草稿中清除';
}

async function syncAllVoices(): Promise<void> {
  await runSync('full');
}
</script>

<template>
  <section class="module-page voice-library-page">
    <VoiceLibraryToolbar
      :search="state.search"
      :loading="state.loading"
      :result-count="filteredVoices.length"
      :total-count="state.voices.length"
      @update:search="setSearch"
      @create="startCreate"
      @sync="syncAllVoices"
      @refresh="loadVoices"
    />

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
          <span>音色指令</span>
          <input v-model="draft.voiceInstruction" placeholder="例如：低沉、稳定、播客旁白" />
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
          <button class="ghost-button" type="button" @click="chooseDraftAudio">选择 wav</button>
        </div>

        <button class="primary-button" type="submit" :disabled="state.saving">保存音色</button>
      </form>
    </div>

    <div v-else class="voice-library-layout">
      <VoiceLibraryRail
        :voices="filteredVoices"
        :selected-voice-name="state.selectedVoiceName"
        :playing-voice-name="state.playingVoiceName"
        @select="selectVoice"
        @preview="previewVoice"
        @set-current="setCurrentVoice"
      />

      <VoiceDetailPanel
        :detail="state.detail"
        :saving="state.saving"
        :playing="state.playingVoiceName === state.detail?.voiceName"
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
