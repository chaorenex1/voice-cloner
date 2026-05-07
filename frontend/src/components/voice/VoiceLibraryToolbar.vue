<script setup lang="ts">
defineProps<{
  search: string;
  loading: boolean;
  resultCount: number;
  totalCount: number;
}>();

defineEmits<{
  'update:search': [value: string];
  create: [];
  sync: [];
  refresh: [];
}>();
</script>

<template>
  <header class="page-toolbar voice-toolbar">
    <div>
      <p class="module-eyebrow">Voice Library</p>
      <!-- <h2>音色管理</h2> -->
      <p class="module-description">浏览、试听、编辑和同步本地可用音色。</p>
    </div>

    <div class="toolbar-actions">
      <label class="search-field">
        <!-- <span>搜索</span> -->
        <input
          :value="search"
          type="search"
          aria-label="搜索音色"
          placeholder="温柔女声 / 自定义 / 播客"
          @input="$emit('update:search', ($event.target as HTMLInputElement).value)"
        />
        <!-- <small>{{
          search ? `匹配 ${resultCount}/${totalCount}` : `共 ${totalCount} 个音色`
        }}</small> -->
      </label>
      <button v-if="search" class="ghost-button" type="button" @click="$emit('update:search', '')">
        清空搜索
      </button>
      <button class="ghost-button" type="button" @click="$emit('create')">新增音色</button>
      <button class="primary-button" type="button" :disabled="loading" @click="$emit('sync')">
        从云端同步
      </button>
      <button class="icon-button" type="button" :disabled="loading" @click="$emit('refresh')">
        刷新
      </button>
    </div>
  </header>
</template>
