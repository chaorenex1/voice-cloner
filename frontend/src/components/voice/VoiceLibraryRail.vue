<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { VoiceSummary } from '../../utils/types/voice';

const props = defineProps<{
  voices: VoiceSummary[];
  selectedVoiceName: string | null;
  playingVoiceName: string | null;
}>();

defineEmits<{
  select: [voiceName: string];
  preview: [voiceName: string];
}>();

const pageSize = 12;
const visibleCount = ref(pageSize);
const listEl = ref<HTMLElement | null>(null);
const loadMoreSentry = ref<HTMLElement | null>(null);
const visibleVoices = computed(() => props.voices.slice(0, visibleCount.value));
const hasMore = computed(() => visibleCount.value < props.voices.length);
let loadMoreObserver: IntersectionObserver | null = null;

watch(
  () => [props.voices.length, props.voices.map((voice) => voice.voiceName).join('|')],
  () => {
    visibleCount.value = Math.min(pageSize, props.voices.length);
    void nextTick(observeLoadMoreSentry);
  }
);

function loadMore(): void {
  visibleCount.value = Math.min(visibleCount.value + pageSize, props.voices.length);
  void nextTick(observeLoadMoreSentry);
}

function handleScroll(event: Event): void {
  const target = event.currentTarget as HTMLElement;
  const reachedBottom = target.scrollTop + target.clientHeight >= target.scrollHeight - 24;
  if (reachedBottom && hasMore.value) {
    loadMore();
  }
}

function observeLoadMoreSentry(): void {
  loadMoreObserver?.disconnect();
  if (
    !listEl.value ||
    !loadMoreSentry.value ||
    !hasMore.value ||
    !('IntersectionObserver' in window)
  ) {
    return;
  }

  loadMoreObserver = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting) && hasMore.value) {
        loadMore();
      }
    },
    {
      root: listEl.value,
      rootMargin: '96px 0px',
      threshold: 0.01,
    }
  );
  loadMoreObserver.observe(loadMoreSentry.value);
}

onMounted(() => {
  void nextTick(observeLoadMoreSentry);
});

onBeforeUnmount(() => {
  loadMoreObserver?.disconnect();
});
</script>

<template>
  <aside class="voice-rail" aria-label="音色浏览栏">
    <div class="rail-heading">
      <span>音色浏览栏</span>
      <strong>{{ voices.length }}</strong>
    </div>

    <div v-if="voices.length" ref="listEl" class="voice-list" @scroll="handleScroll">
      <article
        v-for="voice in visibleVoices"
        :key="voice.voiceName"
        class="voice-card"
        :class="{ 'voice-card--active': voice.voiceName === selectedVoiceName }"
      >
        <button class="voice-card__main" type="button" @click="$emit('select', voice.voiceName)">
          <span class="voice-card__title">
            {{ voice.displayName }}
          </span>
          <span class="voice-card__meta">
            {{
              voice.source === 'preset'
                ? '预置音色'
                : voice.source === 'remote'
                  ? '云端音色'
                  : '自定义音色'
            }}
            ·
            {{ voice.updatedAt }}
          </span>
          <span class="voice-card__preview">{{ voice.referenceTextPreview }}</span>
        </button>

        <div class="voice-card__actions">
          <button type="button" @click="$emit('preview', voice.voiceName)">
            {{ playingVoiceName === voice.voiceName ? '停止' : '试听' }}
          </button>
        </div>
      </article>
      <div v-if="hasMore" ref="loadMoreSentry" class="voice-load-sentry">
        <button class="load-more-button" type="button" @click="loadMore">下滑加载更多音色</button>
      </div>
    </div>

    <div v-else class="empty-state">
      <span>没有匹配音色</span>
      <p>换个关键词，或新增一个自定义音色。</p>
    </div>
  </aside>
</template>
