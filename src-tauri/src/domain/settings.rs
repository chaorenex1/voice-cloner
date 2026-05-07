use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendConfig {
    pub provider_name: String,
    pub base_url: String,
    pub api_key_ref: Option<String>,
    pub model: Option<String>,
    pub timeout_ms: u64,
    pub region: Option<String>,
    #[serde(default)]
    pub extra_options: BTreeMap<String, String>,
}

impl BackendConfig {
    pub fn local_llm_default() -> Self {
        Self {
            provider_name: "local-llm".into(),
            base_url: "http://127.0.0.1:11434".into(),
            api_key_ref: None,
            model: None,
            timeout_ms: 30_000,
            region: None,
            extra_options: BTreeMap::new(),
        }
    }

    pub fn funspeech_default() -> Self {
        Self {
            provider_name: "funspeech".into(),
            base_url: "http://127.0.0.1:8000".into(),
            api_key_ref: None,
            model: None,
            timeout_ms: 30_000,
            region: None,
            extra_options: BTreeMap::new(),
        }
    }

    pub fn validate(&self, label: &str) -> Result<(), String> {
        if self.provider_name.trim().is_empty() {
            return Err(format!("{label}.providerName is required"));
        }
        if self.base_url.trim().is_empty() {
            return Err(format!("{label}.baseUrl is required"));
        }
        if !(self.base_url.starts_with("http://") || self.base_url.starts_with("https://")) {
            return Err(format!("{label}.baseUrl must start with http:// or https://"));
        }
        if self.timeout_ms == 0 {
            return Err(format!("{label}.timeoutMs must be greater than 0"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub monitor_enabled: bool,
    pub virtual_mic_enabled: bool,
    #[serde(default)]
    pub virtual_mic_device_id: Option<String>,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            input_device_id: None,
            output_device_id: None,
            monitor_enabled: true,
            virtual_mic_enabled: false,
            virtual_mic_device_id: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendSettings {
    pub llm: BackendConfig,
    pub asr: BackendConfig,
    pub tts: BackendConfig,
    pub realtime: BackendConfig,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            llm: BackendConfig::local_llm_default(),
            asr: BackendConfig::funspeech_default(),
            tts: BackendConfig::funspeech_default(),
            realtime: BackendConfig::funspeech_default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettings {
    pub default_voice_name: Option<String>,
    pub default_output_format: String,
    pub default_sample_rate: u32,
    pub audio_frame_ms: u16,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            default_voice_name: None,
            default_output_format: "wav".into(),
            default_sample_rate: 48_000,
            audio_frame_ms: 20,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub device: DeviceSettings,
    pub backend: BackendSettings,
    pub runtime: RuntimeSettings,
}

impl AppSettings {
    pub fn normalize_for_local_save(&mut self) {
        if self.backend.llm.provider_name.trim().is_empty() {
            self.backend.llm.provider_name = BackendConfig::local_llm_default().provider_name;
        }
        let funspeech_provider = BackendConfig::funspeech_default().provider_name;
        for config in [&mut self.backend.asr, &mut self.backend.tts, &mut self.backend.realtime] {
            config.provider_name = funspeech_provider.clone();
            config.model = None;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.backend.llm.validate("backend.llm")?;
        self.backend.asr.validate("backend.asr")?;
        self.backend.tts.validate("backend.tts")?;
        self.backend.realtime.validate("backend.realtime")?;

        if self.runtime.default_output_format.trim().is_empty() {
            return Err("runtime.defaultOutputFormat is required".into());
        }
        if self.runtime.default_sample_rate == 0 {
            return Err("runtime.defaultSampleRate must be greater than 0".into());
        }
        if self.runtime.audio_frame_ms == 0 {
            return Err("runtime.audioFrameMs must be greater than 0".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettingsPatch {
    pub input_device_id: Option<Option<String>>,
    pub output_device_id: Option<Option<String>>,
    pub monitor_enabled: Option<bool>,
    pub virtual_mic_enabled: Option<bool>,
    pub virtual_mic_device_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackendSettingsPatch {
    pub llm: Option<BackendConfig>,
    pub asr: Option<BackendConfig>,
    pub tts: Option<BackendConfig>,
    pub realtime: Option<BackendConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsPatch {
    pub default_voice_name: Option<Option<String>>,
    pub default_output_format: Option<String>,
    pub default_sample_rate: Option<u32>,
    pub audio_frame_ms: Option<u16>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub device: Option<DeviceSettingsPatch>,
    pub backend: Option<BackendSettingsPatch>,
    pub runtime: Option<RuntimeSettingsPatch>,
}

impl AppSettingsPatch {
    pub fn apply_to(self, mut settings: AppSettings) -> AppSettings {
        if let Some(device) = self.device {
            if let Some(input_device_id) = device.input_device_id {
                settings.device.input_device_id = input_device_id;
            }
            if let Some(output_device_id) = device.output_device_id {
                settings.device.output_device_id = output_device_id;
            }
            if let Some(monitor_enabled) = device.monitor_enabled {
                settings.device.monitor_enabled = monitor_enabled;
            }
            if let Some(virtual_mic_enabled) = device.virtual_mic_enabled {
                settings.device.virtual_mic_enabled = virtual_mic_enabled;
            }
            if let Some(virtual_mic_device_id) = device.virtual_mic_device_id {
                settings.device.virtual_mic_device_id = virtual_mic_device_id;
            }
        }

        if let Some(backend) = self.backend {
            if let Some(llm) = backend.llm {
                settings.backend.llm = llm;
            }
            if let Some(asr) = backend.asr {
                settings.backend.asr = asr;
            }
            if let Some(tts) = backend.tts {
                settings.backend.tts = tts;
            }
            if let Some(realtime) = backend.realtime {
                settings.backend.realtime = realtime;
            }
        }

        if let Some(runtime) = self.runtime {
            if let Some(default_voice_name) = runtime.default_voice_name {
                settings.runtime.default_voice_name = default_voice_name;
            }
            if let Some(default_output_format) = runtime.default_output_format {
                settings.runtime.default_output_format = default_output_format;
            }
            if let Some(default_sample_rate) = runtime.default_sample_rate {
                settings.runtime.default_sample_rate = default_sample_rate;
            }
            if let Some(audio_frame_ms) = runtime.audio_frame_ms {
                settings.runtime.audio_frame_ms = audio_frame_ms;
            }
        }

        settings
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSettings, AppSettingsPatch, DeviceSettingsPatch, RuntimeSettingsPatch};

    #[test]
    fn default_settings_match_mvp_boundaries() {
        let settings = AppSettings::default();

        assert_eq!(settings.backend.llm.provider_name, "local-llm");
        assert_eq!(settings.backend.asr.provider_name, "funspeech");
        assert_eq!(settings.backend.tts.provider_name, "funspeech");
        assert_eq!(settings.backend.realtime.provider_name, "funspeech");
        assert_eq!(settings.runtime.default_output_format, "wav");
        assert_eq!(settings.runtime.audio_frame_ms, 20);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn settings_patch_updates_nested_fields_without_losing_defaults() {
        let patch = AppSettingsPatch {
            device: Some(DeviceSettingsPatch {
                input_device_id: Some(Some("mic-1".into())),
                virtual_mic_enabled: Some(true),
                virtual_mic_device_id: Some(Some("virtual-mic-1".into())),
                ..Default::default()
            }),
            runtime: Some(RuntimeSettingsPatch {
                default_voice_name: Some(Some("narrator".into())),
                ..Default::default()
            }),
            ..Default::default()
        };

        let settings = patch.apply_to(AppSettings::default());

        assert_eq!(settings.device.input_device_id.as_deref(), Some("mic-1"));
        assert_eq!(settings.runtime.default_voice_name.as_deref(), Some("narrator"));
        assert_eq!(settings.backend.tts.provider_name, "funspeech");
        assert!(settings.device.virtual_mic_enabled);
        assert_eq!(settings.device.virtual_mic_device_id.as_deref(), Some("virtual-mic-1"));
    }

    #[test]
    fn settings_validation_rejects_invalid_backend_url() {
        let mut settings = AppSettings::default();
        settings.backend.realtime.base_url = "localhost:8000".into();

        assert!(settings.validate().unwrap_err().contains("baseUrl"));
    }
}
