<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import OfflineVoicePage from './pages/OfflineVoicePage.vue';
import RealtimeVoicePage from './pages/RealtimeVoicePage.vue';
import SettingsPage from './pages/SettingsPage.vue';
import VoiceLibraryPage from './pages/VoiceLibraryPage.vue';
import { getSettings } from './services/tauri/settings';
import type { OfflineJob } from './utils/types/offline';

interface AppSummary {
  name: string;
  version: string;
  status: string;
  message: string;
}

interface NavItem {
  key: string;
  label: string;
  description: string;
}

const defaultNavItem: NavItem = {
  key: 'voices',
  label: '音色管理',
  description: '统一承载音色资料、样本与基础管理入口。',
};

const navItems: NavItem[] = [
  defaultNavItem,
  {
    key: 'realtime',
    label: '实时',
    description: '承载实时变声、监听与会话控制页面。',
  },
  {
    key: 'offline',
    label: '离线',
    description: '承载文件转换、导出队列与历史记录页面。',
  },
  {
    key: 'design',
    label: '音色设计',
    description: '承载音色创建、调参和实验工作区。',
  },
  {
    key: 'settings',
    label: '设置',
    description: '承载设备、模型、存储与应用偏好设置。',
  },
];

const appSummary = ref<AppSummary | null>(null);
const activeNavKey = ref(defaultNavItem.key);
const settingsReturnNavKey = ref<string | null>(null);
const backendState = ref('正在连接桌面运行时...');
const notifiedOfflineJobIds = new Set<string>();
let stopOfflineNotificationListener: UnlistenFn | null = null;

const currentModule = computed(
  () => navItems.find((item) => item.key === activeNavKey.value) ?? defaultNavItem
);

const hasImplementedPage = computed(() =>
  ['voices', 'realtime', 'offline', 'settings'].includes(activeNavKey.value)
);

onMounted(async () => {
  void startOfflineCompletionNotifications();
  void getSettings().catch(() => undefined);
  try {
    appSummary.value = await invoke<AppSummary>('get_app_summary');
    backendState.value = '已连接';
  } catch (_error) {
    backendState.value = '前端预览模式';
  }
});

onBeforeUnmount(() => {
  stopOfflineNotificationListener?.();
  stopOfflineNotificationListener = null;
});

function openDeviceSettingsFromRealtime(): void {
  settingsReturnNavKey.value = 'realtime';
  activeNavKey.value = 'settings';
}

function returnFromSettings(): void {
  activeNavKey.value = settingsReturnNavKey.value ?? defaultNavItem.key;
  settingsReturnNavKey.value = null;
}

async function startOfflineCompletionNotifications(): Promise<void> {
  if (stopOfflineNotificationListener) {
    return;
  }
  void ensureNotificationPermission();
  try {
    stopOfflineNotificationListener = await listen<OfflineJob>('offline-job-updated', (event) => {
      const job = event.payload;
      if (job.status !== 'completed' || notifiedOfflineJobIds.has(job.jobId)) {
        return;
      }
      notifiedOfflineJobIds.add(job.jobId);
      void showOfflineCompletionNotification(job);
    });
  } catch (_error) {
    stopOfflineNotificationListener = null;
  }
}

async function ensureNotificationPermission(): Promise<void> {
  if ('Notification' in window && Notification.permission === 'default') {
    await Notification.requestPermission();
  }
}

async function showOfflineCompletionNotification(job: OfflineJob): Promise<void> {
  if (!('Notification' in window)) {
    return;
  }
  if (Notification.permission !== 'granted') {
    return;
  }
  const name = job.inputFileName ?? (job.inputType === 'text' ? '文本任务' : '音频任务');
  new Notification('Offline Voice 完成', {
    body: `${name} 已完成离线转换。`,
    tag: `offline-job-${job.jobId}`,
  });
}
</script>

<template>
  <div class="app-shell">
    <aside class="app-sider" aria-label="主导航">
      <div class="brand-block">
        <!-- <span class="brand-mark">VC</span> -->
        <div>
          <p class="brand-kicker">Voice Cloner</p>
          <!-- <h1>声音克隆工作台</h1> -->
        </div>
      </div>

      <nav class="primary-nav" aria-label="应用模块">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="nav-item"
          :class="{ 'nav-item--active': item.key === activeNavKey }"
          type="button"
          @click="
            activeNavKey = item.key;
            settingsReturnNavKey = null;
          "
        >
          <span class="nav-item__label">{{ item.label }}</span>
        </button>
      </nav>

      <div class="sider-status" aria-label="全局状态">
        <span class="status-dot" :class="{ 'status-dot--online': appSummary }"></span>
        <div>
          <p>{{ backendState }}</p>
          <span>{{
            appSummary ? `${appSummary.name} v${appSummary.version}` : '等待运行时元信息'
          }}</span>
        </div>
      </div>
    </aside>

    <main
      class="main-content"
      :class="{ 'main-content--locked': ['realtime', 'offline'].includes(activeNavKey) }"
      aria-live="polite"
    >
      <VoiceLibraryPage v-if="activeNavKey === 'voices'" />
      <RealtimeVoicePage
        v-else-if="activeNavKey === 'realtime'"
        @open-device-settings="openDeviceSettingsFromRealtime"
      />
      <OfflineVoicePage v-else-if="activeNavKey === 'offline'" />
      <SettingsPage
        v-else-if="activeNavKey === 'settings'"
        :return-target="settingsReturnNavKey"
        @back="returnFromSettings"
      />

      <section
        v-else
        class="module-page"
        :class="{ 'module-page--placeholder': !hasImplementedPage }"
        :aria-labelledby="`${currentModule.key}-title`"
      >
        <p class="module-eyebrow">Main Content</p>
        <h2 :id="`${currentModule.key}-title`">{{ currentModule.label }}</h2>
        <p class="module-description">{{ currentModule.description }}</p>

        <div class="content-placeholder">
          <span class="placeholder-label">页面内容占位</span>
          <p>
            应用层只提供统一右侧容器；该模块的标题、摘要、操作和业务内容后续在页面内部自行组织。
          </p>
        </div>
      </section>
    </main>
  </div>
</template>
