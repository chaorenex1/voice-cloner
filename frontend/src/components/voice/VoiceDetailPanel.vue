<script setup lang="ts">
import { computed } from 'vue';
import type { VoiceDetail } from '../../utils/types/voice';

const props = defineProps<{
  detail: VoiceDetail | null;
  saving: boolean;
  playing: boolean;
  operation: string | null;
}>();

defineEmits<{
  updateDetail: [patch: Partial<VoiceDetail>];
  preview: [];
  save: [];
  delete: [];
  uploadAudio: [];
  clearAudio: [];
}>();

const audioBusy = computed(
  () => props.operation === 'uploadingAudio' || props.operation === 'recognizingAudio'
);
const deleting = computed(() => props.operation === 'deletingVoice');
const savingVoice = computed(() => props.operation === 'savingVoice');
</script>

<template>
  <section class="voice-stage" aria-label="当前音色舞台">
    <div v-if="detail" class="voice-detail">
      <div class="voice-visual">
        <div class="wave-orb">
          <span></span>
          <span></span>
          <span></span>
          <span></span>
          <span></span>
        </div>
        <div>
          <p>{{ detail.source === 'preset' ? 'Preset Voice' : 'Custom Voice' }}</p>
          <strong>{{ detail.displayName }}</strong>
        </div>
      </div>

      <div class="detail-header">
        <div>
          <p class="module-eyebrow">当前音色舞台</p>
          <h3>{{ detail.displayName }}</h3>
          <span class="sync-badge" :data-status="detail.syncStatus">
            {{ detail.syncStatus }}
          </span>
        </div>
        <div class="detail-actions">
          <button class="ghost-button" type="button" @click="$emit('preview')">
            {{ playing ? '停止试听' : '试听' }}
          </button>
          <button
            class="primary-button"
            type="button"
            :class="{ 'button--busy': savingVoice }"
            :disabled="saving || !detail.editable"
            @click="$emit('save')"
          >
            {{ savingVoice ? '保存中' : '保存修改' }}
          </button>
          <button
            class="danger-button"
            type="button"
            :class="{ 'button--busy': deleting }"
            :disabled="saving || !detail.editable"
            @click="$emit('delete')"
          >
            {{ deleting ? '删除中' : '删除' }}
          </button>
        </div>
      </div>

      <div class="form-grid">
        <label class="form-field">
          <span>名称</span>
          <input
            :value="detail.displayName"
            :readonly="!detail.editable"
            @input="
              $emit('updateDetail', { displayName: ($event.target as HTMLInputElement).value })
            "
          />
        </label>

        <label class="form-field form-field--wide">
          <span>参考文本</span>
          <textarea
            :value="detail.referenceText"
            :readonly="!detail.editable"
            rows="5"
            @input="
              $emit('updateDetail', {
                referenceText: ($event.target as HTMLTextAreaElement).value,
                referenceTextPreview: ($event.target as HTMLTextAreaElement).value.slice(0, 42),
              })
            "
          ></textarea>
        </label>
      </div>

      <div class="audio-panel">
        <div>
          <p class="module-eyebrow">参考音频</p>
          <strong>{{ detail.referenceAudioFileName ?? '尚未上传参考音频' }}</strong>
          <span>只支持 wav 参考音频，试听会使用设置页选择的输出设备。</span>
        </div>
        <div class="audio-actions">
          <button
            class="ghost-button"
            type="button"
            :class="{ 'button--busy': audioBusy }"
            :disabled="!detail.editable || audioBusy"
            @click="$emit('uploadAudio')"
          >
            {{
              operation === 'recognizingAudio'
                ? '识别中'
                : operation === 'uploadingAudio'
                  ? '上传中'
                  : '重新上传'
            }}
          </button>
          <button
            class="ghost-button"
            type="button"
            :disabled="!detail.editable || audioBusy"
            @click="$emit('clearAudio')"
          >
            清除
          </button>
        </div>
      </div>

      <footer class="detail-status">
        {{
          detail.isCurrent
            ? '当前音色已载入，可直接用于实时变声。'
            : '选择“设为当前音色”后用于实时变声。'
        }}
      </footer>
    </div>

    <div v-else class="empty-state empty-state--large">
      <span>请选择音色</span>
      <p>左侧切换音色后，这里会展示参考文本、参考音频和同步状态。</p>
    </div>
  </section>
</template>
