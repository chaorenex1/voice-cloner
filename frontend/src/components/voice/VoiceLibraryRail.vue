<script setup lang="ts">
import type { VoiceSummary } from '../../utils/types/voice';

defineProps<{
  voices: VoiceSummary[];
  selectedVoiceName: string | null;
}>();

defineEmits<{
  select: [voiceName: string];
  preview: [voiceName: string];
  setCurrent: [voiceName: string];
}>();
</script>

<template>
  <aside class="voice-rail" aria-label="音色浏览栏">
    <div class="rail-heading">
      <span>音色浏览栏</span>
      <strong>{{ voices.length }}</strong>
    </div>

    <div v-if="voices.length" class="voice-list">
      <article
        v-for="voice in voices"
        :key="voice.voiceName"
        class="voice-card"
        :class="{ 'voice-card--active': voice.voiceName === selectedVoiceName }"
      >
        <button class="voice-card__main" type="button" @click="$emit('select', voice.voiceName)">
          <span class="voice-card__title">
            {{ voice.displayName }}
            <small v-if="voice.isCurrent">当前</small>
          </span>
          <span class="voice-card__meta">
            {{ voice.source === 'preset' ? '预置音色' : '自定义音色' }} · {{ voice.updatedAt }}
          </span>
          <span class="voice-card__preview">{{ voice.referenceTextPreview }}</span>
        </button>

        <div class="voice-card__actions">
          <button type="button" @click="$emit('preview', voice.voiceName)">试听</button>
          <button type="button" @click="$emit('setCurrent', voice.voiceName)">设为当前音色</button>
        </div>
      </article>
    </div>

    <div v-else class="empty-state">
      <span>没有匹配音色</span>
      <p>换个关键词，或新增一个自定义音色。</p>
    </div>
  </aside>
</template>
