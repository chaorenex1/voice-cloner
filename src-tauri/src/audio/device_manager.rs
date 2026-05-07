use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

use crate::app::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioDeviceKind {
    Input,
    Output,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: AudioDeviceKind,
    pub is_default: bool,
}

#[derive(Debug, Default)]
pub struct AudioDeviceManager;

impl AudioDeviceManager {
    pub fn list_input_devices(&self) -> AppResult<Vec<AudioDeviceInfo>> {
        self.list_devices(AudioDeviceKind::Input)
    }

    pub fn list_output_devices(&self) -> AppResult<Vec<AudioDeviceInfo>> {
        self.list_devices(AudioDeviceKind::Output)
    }

    pub fn default_devices(&self) -> AppResult<DefaultAudioDevices> {
        let host = cpal::default_host();
        Ok(DefaultAudioDevices {
            input: device_name(host.default_input_device()),
            output: device_name(host.default_output_device()),
        })
    }

    fn list_devices(&self, kind: AudioDeviceKind) -> AppResult<Vec<AudioDeviceInfo>> {
        let host = cpal::default_host();
        let default_name = match kind {
            AudioDeviceKind::Input => device_name(host.default_input_device()),
            AudioDeviceKind::Output => device_name(host.default_output_device()),
        };
        let devices = match kind {
            AudioDeviceKind::Input => host
                .input_devices()
                .map_err(|error| AppError::audio(error.to_string()))?
                .collect::<Vec<_>>(),
            AudioDeviceKind::Output => host
                .output_devices()
                .map_err(|error| AppError::audio(error.to_string()))?
                .collect::<Vec<_>>(),
        };

        Ok(devices
            .into_iter()
            .enumerate()
            .map(|(index, device)| {
                let name = device
                    .name()
                    .unwrap_or_else(|_| format!("Unknown {kind:?} device {index}"));
                let is_default = default_name.as_deref() == Some(name.as_str());
                AudioDeviceInfo {
                    id: stable_device_id(kind, index, &name),
                    name,
                    kind,
                    is_default,
                }
            })
            .collect())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAudioDevices {
    pub input: Option<String>,
    pub output: Option<String>,
}

fn device_name(device: Option<cpal::Device>) -> Option<String> {
    device.and_then(|device| device.name().ok())
}

fn stable_device_id(kind: AudioDeviceKind, index: usize, name: &str) -> String {
    let normalized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let prefix = match kind {
        AudioDeviceKind::Input => "input",
        AudioDeviceKind::Output => "output",
    };
    format!("{prefix}-{index}-{normalized}")
}

#[cfg(test)]
mod tests {
    use super::{stable_device_id, AudioDeviceKind};

    #[test]
    fn stable_device_id_normalizes_names_for_ipc_storage() {
        assert_eq!(
            stable_device_id(AudioDeviceKind::Input, 2, "USB Mic (Main)"),
            "input-2-usb-mic--main"
        );
    }
}
