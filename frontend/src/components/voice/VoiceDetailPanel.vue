<script setup lang="ts">
import type { VoiceDetail } from '../../utils/types/voice';

defineProps<{
  detail: VoiceDetail | null;
  saving: boolean;
}>();

defineEmits<{
  updateDetail: [patch: Partial<VoiceDetail>];
  preview: [];
  save: [];
  delete: [];
  uploadAudio: [];
  clearAudio: [];
}>();
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
          <button class="ghost-button" type="button" @click="$emit('preview')">试听</button>
          <button
            class="primary-button"
            type="button"
            :disabled="saving || !detail.editable"
            @click="$emit('save')"
          >
            保存修改
          </button>
          <button
            class="danger-button"
            type="button"
            :disabled="!detail.editable"
            @click="$emit('delete')"
          >
            删除
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

        <label class="form-field">
          <span>音色指令</span>
          <input
            :value="detail.voiceInstruction ?? ''"
            :readonly="!detail.editable"
            placeholder="描述音色气质、语速或使用场景"
            @input="
              $emit('updateDetail', {
                voiceInstruction: ($event.target as HTMLInputElement).value,
              })
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
          <strong>{{ detail.referenceAudioPath ?? '尚未上传参考音频' }}</strong>
          <span>波形预览区会在接入真实音频后展示实际样本。</span>
        </div>
        <div class="audio-actions">
          <button
            class="ghost-button"
            type="button"
            :disabled="!detail.editable"
            @click="$emit('uploadAudio')"
          >
            重新上传
          </button>
          <button
            class="ghost-button"
            type="button"
            :disabled="!detail.editable"
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
