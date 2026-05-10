<script setup lang="ts">
import type { AudioDeviceSnapshot, DeviceSettings } from '../../utils/types/settings';

defineProps<{
  devices: AudioDeviceSnapshot | null;
  settings: DeviceSettings | null;
}>();

defineEmits<{
  update: [patch: Partial<DeviceSettings>];
  commit: [];
}>();

function normalizeDeviceId(value: string): string | null {
  return value || null;
}
</script>

<template>
  <section class="settings-card">
    <div class="settings-card__header">
      <p class="module-eyebrow">Device Settings</p>
      <!-- <h3>设备主卡</h3> -->
      <!-- <span>选择实时链路默认输入、输出与虚拟麦克风。</span> -->
    </div>

    <div v-if="settings && devices" class="settings-form">
      <label class="form-field">
        <span>输入设备</span>
        <select
          :value="settings.inputDeviceId ?? ''"
          @change="
            $emit('update', {
              inputDeviceId: normalizeDeviceId(($event.target as HTMLSelectElement).value),
            })
          "
          @blur="$emit('commit')"
        >
          <option value="">系统默认输入设备</option>
          <option v-for="device in devices.inputDevices" :key="device.id" :value="device.id">
            {{ device.name }}{{ device.isDefault ? '（默认）' : '' }}
          </option>
        </select>
      </label>

      <label class="form-field">
        <span>输出设备</span>
        <select
          :value="settings.outputDeviceId ?? ''"
          @change="
            $emit('update', {
              outputDeviceId: normalizeDeviceId(($event.target as HTMLSelectElement).value),
            })
          "
          @blur="$emit('commit')"
        >
          <option value="">系统默认输出设备</option>
          <option v-for="device in devices.outputDevices" :key="device.id" :value="device.id">
            {{ device.name }}{{ device.isDefault ? '（默认）' : '' }}
          </option>
        </select>
      </label>

      <label class="toggle-field">
        <input
          :checked="settings.monitorEnabled"
          type="checkbox"
          @change="
            $emit('update', {
              monitorEnabled: ($event.target as HTMLInputElement).checked,
            })
          "
          @blur="$emit('commit')"
        />
        <span>启用本机监听</span>
      </label>

      <label class="toggle-field">
        <input
          :checked="settings.virtualMicEnabled"
          type="checkbox"
          @change="
            $emit('update', {
              virtualMicEnabled: ($event.target as HTMLInputElement).checked,
            })
          "
          @blur="$emit('commit')"
        />
        <span>启用虚拟麦克风</span>
      </label>

      <label class="form-field">
        <span>虚拟麦克风</span>
        <select
          :disabled="!settings.virtualMicEnabled"
          :value="settings.virtualMicDeviceId ?? ''"
          @change="
            $emit('update', {
              virtualMicDeviceId: normalizeDeviceId(($event.target as HTMLSelectElement).value),
            })
          "
          @blur="$emit('commit')"
        >
          <option v-for="device in devices.outputDevices" :key="device.id" :value="device.id">
            {{ device.name }}{{ device.isDefault ? '（默认）' : '' }}
          </option>
        </select>
      </label>
    </div>

    <div v-else class="empty-state">
      <span>正在加载设备</span>
      <p>设备列表将由 Tauri 命令提供。</p>
    </div>
  </section>
</template>
