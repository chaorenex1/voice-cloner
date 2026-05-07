<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue';
import VoiceDetailPanel from '../components/voice/VoiceDetailPanel.vue';
import VoiceLibraryRail from '../components/voice/VoiceLibraryRail.vue';
import VoiceLibraryToolbar from '../components/voice/VoiceLibraryToolbar.vue';
import { useVoiceLibraryStore } from '../stores/voice-library.store';

const {
  state,
  filteredVoices,
  loadVoices,
  selectVoice,
  updateDetail,
  saveSelectedVoice,
  setCurrentVoice,
  createLocalVoice,
  runSync,
} = useVoiceLibraryStore();

const isCreating = ref(false);
const draft = reactive({
  displayName: '',
  referenceText: '',
  referenceAudioPath: '',
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

function submitDraft(): void {
  if (!draft.displayName.trim() || !draft.referenceText.trim()) {
    state.lastMessage = '新增音色需要名称和参考文本';
    return;
  }

  createLocalVoice({
    displayName: draft.displayName,
    referenceText: draft.referenceText,
    referenceAudioPath: draft.referenceAudioPath,
  });
  draft.displayName = '';
  draft.referenceText = '';
  draft.referenceAudioPath = '';
  isCreating.value = false;
}

function previewVoice(voiceName?: string): void {
  const target = voiceName ?? state.detail?.voiceName;
  state.lastMessage = target ? `${target} 试听请求已进入队列` : '请选择要试听的音色';
}

function uploadAudioPlaceholder(): void {
  updateDetail({
    hasReferenceAudio: true,
    referenceAudioPath: '~/voice-cloner/library/custom-voices/new-reference.wav',
  });
  state.lastMessage = '参考音频上传入口已占位';
}

function clearAudio(): void {
  updateDetail({
    hasReferenceAudio: false,
    referenceAudioPath: undefined,
  });
  state.lastMessage = '参考音频已从当前草稿中清除';
}

async function syncAllVoices(): Promise<void> {
  await runSync('full');
}
</script>

<template>
  <section class="module-page voice-library-page">
    <VoiceLibraryToolbar
      v-model:search="state.search"
      :loading="state.loading"
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
          <li>录音尽量干净，避免背景音乐和混响。</li>
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

        <label class="form-field">
          <span>参考音频 *</span>
          <input
            v-model="draft.referenceAudioPath"
            placeholder="~/voice-cloner/library/custom-voices/sample.wav"
          />
        </label>

        <button class="primary-button" type="submit">保存音色</button>
      </form>
    </div>

    <div v-else class="voice-library-layout">
      <VoiceLibraryRail
        :voices="filteredVoices"
        :selected-voice-name="state.selectedVoiceName"
        @select="selectVoice"
        @preview="previewVoice"
        @set-current="setCurrentVoice"
      />

      <VoiceDetailPanel
        :detail="state.detail"
        :saving="state.saving"
        @update-detail="updateDetail"
        @preview="previewVoice"
        @save="saveSelectedVoice"
        @delete="state.lastMessage = '删除命令将在 Rust 本地读写接入后启用'"
        @upload-audio="uploadAudioPlaceholder"
        @clear-audio="clearAudio"
      />
    </div>

    <footer class="page-status-bar">
      <span>{{ state.lastMessage }}</span>
    </footer>
  </section>
</template>
