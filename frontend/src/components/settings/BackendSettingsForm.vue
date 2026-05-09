<script setup lang="ts">
import type {
  AppSettings,
  BackendEndpointConfig,
  BackendSettings,
} from '../../utils/types/settings';

type BackendKey = keyof BackendSettings;

defineProps<{
  settings: AppSettings['backend'] | null;
}>();

defineEmits<{
  update: [key: BackendKey, patch: Partial<BackendEndpointConfig>];
  updateFunSpeech: [patch: Partial<BackendEndpointConfig>];
  commit: [];
}>();

function nullableText(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function funSpeechConfig(settings: AppSettings['backend']): BackendEndpointConfig {
  return settings.realtime;
}
</script>

<template>
  <div v-if="settings" class="backend-grid">
    <section class="settings-card">
      <div class="settings-card__header">
        <p class="module-eyebrow">LLM Backend</p>
        <span>用于提示词生成、文案改写与语音设计辅助。</span>
      </div>

      <div class="settings-form">
        <label class="form-field">
          <span>Base URL</span>
          <input
            :value="settings.llm.baseUrl"
            @input="
              $emit('update', 'llm', {
                baseUrl: ($event.target as HTMLInputElement).value,
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>Model</span>
          <input
            :value="settings.llm.model ?? ''"
            @input="
              $emit('update', 'llm', {
                model: nullableText(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>API Key Ref</span>
          <input
            :value="settings.llm.apiKeyRef ?? ''"
            @input="
              $emit('update', 'llm', {
                apiKeyRef: nullableText(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>Timeout(ms)</span>
          <input
            :value="settings.llm.timeoutMs"
            inputmode="numeric"
            @input="
              $emit('update', 'llm', {
                timeoutMs: Number(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>
      </div>
    </section>

    <section class="settings-card">
      <div class="settings-card__header">
        <p class="module-eyebrow">FunSpeech Backend</p>
        <span>统一配置 ASR、TTS 与实时变声连接；FunSpeech 不设置模型。</span>
      </div>

      <div class="settings-form">
        <label class="form-field">
          <span>Base URL</span>
          <input
            :value="funSpeechConfig(settings).baseUrl"
            @input="
              $emit('updateFunSpeech', {
                baseUrl: ($event.target as HTMLInputElement).value,
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>API Key Ref</span>
          <input
            :value="funSpeechConfig(settings).apiKeyRef ?? ''"
            @input="
              $emit('updateFunSpeech', {
                apiKeyRef: nullableText(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>Timeout(ms)</span>
          <input
            :value="funSpeechConfig(settings).timeoutMs"
            inputmode="numeric"
            @input="
              $emit('updateFunSpeech', {
                timeoutMs: Number(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>
      </div>
    </section>
  </div>

  <div v-else class="empty-state empty-state--large">
    <span>正在加载后端配置</span>
    <p>配置将从本地 settings/app-settings.json 读取。</p>
  </div>
</template>
