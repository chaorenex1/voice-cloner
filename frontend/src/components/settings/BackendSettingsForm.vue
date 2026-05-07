<script setup lang="ts">
import type { AppSettings, BackendEndpointConfig } from '../../utils/types/settings';

defineProps<{
  settings: AppSettings['backends'] | null;
}>();

defineEmits<{
  update: [key: keyof AppSettings['backends'], patch: Partial<BackendEndpointConfig>];
}>();
</script>

<template>
  <div v-if="settings" class="backend-grid">
    <section class="settings-card">
      <div class="settings-card__header">
        <p class="module-eyebrow">FunSpeech</p>
        <!-- <h3>FunSpeech 卡</h3> -->
        <!-- <span>用于同步音色与调用 voice_manager。</span> -->
      </div>

      <div class="settings-form">
        <label class="form-field">
          <span>Base URL</span>
          <input
            :value="settings.funspeech.baseUrl"
            @input="
              $emit('update', 'funspeech', {
                baseUrl: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
        <label class="form-field">
          <span>API Key </span>
          <input
            :value="settings.funspeech.apiKeyRef ?? ''"
            @input="
              $emit('update', 'funspeech', {
                apiKeyRef: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
        <label class="form-field">
          <span>Timeout(ms)</span>
          <input
            :value="settings.funspeech.timeoutMs"
            inputmode="numeric"
            @input="
              $emit('update', 'funspeech', {
                timeoutMs: Number(($event.target as HTMLInputElement).value),
              })
            "
          />
        </label>
      </div>
    </section>

    <section class="settings-card">
      <div class="settings-card__header">
        <p class="module-eyebrow">LLM Backend</p>
        <!-- <h3>LLM / 语音后端卡</h3> -->
        <!-- <span>用于后续提示词、ASR/TTS 或实时后端配置。</span> -->
      </div>

      <div class="settings-form">
        <label class="form-field">
          <span>LLM Base URL</span>
          <input
            :value="settings.llm.baseUrl"
            @input="
              $emit('update', 'llm', {
                baseUrl: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
        <label class="form-field">
          <span>LLM Model</span>
          <input
            :value="settings.llm.model ?? ''"
            @input="
              $emit('update', 'llm', {
                model: ($event.target as HTMLInputElement).value,
              })
            "
          />
        </label>
        <label class="form-field">
          <span>API Key </span>
          <input
            :value="settings.llm.apiKeyRef ?? ''"
            @input="
              $emit('update', 'llm', {
                apiKeyRef: ($event.target as HTMLInputElement).value,
              })
            "
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
