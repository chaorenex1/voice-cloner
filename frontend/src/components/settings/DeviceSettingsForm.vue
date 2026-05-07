<script setup lang="ts">
import type { AudioDeviceSnapshot, DeviceSettings } from '../../utils/types/settings';

defineProps<{
  devices: AudioDeviceSnapshot | null;
  settings: DeviceSettings | null;
}>();

defineEmits<{
  update: [patch: Partial<DeviceSettings>];
}>();
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
          @change="$emit('update', { inputDeviceId: ($event.target as HTMLSelectElement).value })"
        >
          <option v-for="device in devices.inputDevices" :key="device.id" :value="device.id">
            {{ device.label }}
          </option>
        </select>
      </label>

      <label class="form-field">
        <span>输出设备</span>
        <select
          :value="settings.outputDeviceId ?? ''"
          @change="$emit('update', { outputDeviceId: ($event.target as HTMLSelectElement).value })"
        >
          <option v-for="device in devices.outputDevices" :key="device.id" :value="device.id">
            {{ device.label }}
          </option>
        </select>
      </label>

      <label class="form-field">
        <span>虚拟麦克风</span>
        <select
          :value="settings.virtualMicDeviceId ?? ''"
          @change="
            $emit('update', { virtualMicDeviceId: ($event.target as HTMLSelectElement).value })
          "
        >
          <option v-for="device in devices.virtualMicDevices" :key="device.id" :value="device.id">
            {{ device.label }}
          </option>
        </select>
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
        />
        <span>启用虚拟麦克风</span>
      </label>
    </div>

    <div v-else class="empty-state">
      <span>正在加载设备</span>
      <p>设备列表将由 Tauri 命令提供。</p>
    </div>
  </section>
</template>
