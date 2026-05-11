<script setup lang="ts">
import type {
  AppSettings,
  BackendEndpointConfig,
  BackendSettings,
  McpSettings,
} from '../../utils/types/settings';

type BackendKey = Exclude<keyof BackendSettings, 'mcp'>;

defineProps<{
  settings: AppSettings['backend'] | null;
}>();

defineEmits<{
  update: [key: BackendKey, patch: Partial<BackendEndpointConfig>];
  updateFunSpeech: [patch: Partial<BackendEndpointConfig>];
  updateMcp: [patch: Partial<McpSettings>];
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
  <div v-if="settings" class="backend-grid backend-grid--single">
    <section class="settings-card backend-settings-card">
      <div class="settings-card__header">
        <p class="module-eyebrow">Backend Services</p>
        <span>集中配置 LLM、FunSpeech 与本机 MCP Streamable HTTP 服务。</span>
      </div>

      <div class="backend-settings-group">
        <div class="backend-settings-group__heading">
          <strong>LLM</strong>
          <span>用于提示词生成、文案改写与语音设计辅助。</span>
        </div>
        <label class="form-field">
          <span>LLM Base URL</span>
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
          <span>LLM Model</span>
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
          <span>LLM API Key Ref</span>
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
          <span>LLM Timeout(ms)</span>
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

      <div class="backend-settings-group">
        <div class="backend-settings-group__heading">
          <strong>FunSpeech</strong>
          <span>统一配置 ASR、TTS 与实时变声连接；FunSpeech 不设置模型。</span>
        </div>
        <label class="form-field">
          <span>FunSpeech Base URL</span>
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
          <span>FunSpeech API Key Ref</span>
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
          <span>FunSpeech Timeout(ms)</span>
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

      <div class="backend-settings-group backend-settings-group--mcp">
        <div class="backend-settings-group__heading">
          <strong>MCP</strong>
          <span>Streamable HTTP 服务，暴露离线变声、人声分离、结果资源与使用说明。</span>
        </div>

        <label class="toggle-field form-field--wide">
          <input
            :checked="settings.mcp.enabled"
            type="checkbox"
            @change="
              $emit('updateMcp', {
                enabled: ($event.target as HTMLInputElement).checked,
              });
              $emit('commit');
            "
          />
          <span>启用 MCP Streamable HTTP 服务</span>
        </label>

        <label class="form-field">
          <span>MCP Host</span>
          <input
            :value="settings.mcp.host"
            @input="
              $emit('updateMcp', {
                host: ($event.target as HTMLInputElement).value,
              })
            "
            @blur="$emit('commit')"
          />
          <small>仅允许 127.0.0.1 或 localhost。</small>
        </label>

        <label class="form-field">
          <span>MCP Port</span>
          <input
            :value="settings.mcp.port"
            inputmode="numeric"
            @input="
              $emit('updateMcp', {
                port: Number(($event.target as HTMLInputElement).value),
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <label class="form-field">
          <span>MCP Path</span>
          <input
            :value="settings.mcp.path"
            @input="
              $emit('updateMcp', {
                path: ($event.target as HTMLInputElement).value,
              })
            "
            @blur="$emit('commit')"
          />
        </label>

        <p class="backend-mcp-endpoint">
          Endpoint: http://{{ settings.mcp.host }}:{{ settings.mcp.port }}{{ settings.mcp.path }}
        </p>
      </div>
    </section>
  </div>

  <div v-else class="empty-state empty-state--large">
    <span>正在加载后端配置</span>
    <p>配置将从本地 settings/app-settings.json 读取。</p>
  </div>
</template>
