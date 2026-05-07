use std::sync::RwLock;

use crate::{
    app::error::{AppError, AppResult},
    audio::{
        device_manager::AudioDeviceManager,
        frame::{AudioFrame, PcmFormat},
    },
};

pub trait VirtualMicAdapter: Send + Sync {
    fn is_available(&self) -> bool;
    fn start(&self, format: PcmFormat) -> AppResult<()>;
    fn write_frame(&self, frame: &AudioFrame) -> AppResult<()>;
    fn stop(&self) -> AppResult<()>;
}

#[derive(Debug, Default)]
pub struct SelectableVirtualMicAdapter {
    target_device_id: RwLock<Option<String>>,
    running_format: RwLock<Option<PcmFormat>>,
    accepted_frame_count: RwLock<u64>,
}

impl SelectableVirtualMicAdapter {
    pub fn set_target_device_id(&self, target_device_id: Option<String>) {
        *self.target_device_id.write().expect("virtual mic lock poisoned") = target_device_id;
    }

    pub fn target_device_id(&self) -> Option<String> {
        self.target_device_id.read().expect("virtual mic lock poisoned").clone()
    }

    pub fn accepted_frame_count(&self) -> u64 {
        *self.accepted_frame_count.read().expect("virtual mic lock poisoned")
    }

    fn validate_target_device(&self) -> AppResult<String> {
        let Some(target_device_id) = self.target_device_id() else {
            return Err(AppError::audio(
                "virtualMicDeviceId is required when virtual microphone is enabled",
            ));
        };

        let devices = AudioDeviceManager::default().list_input_devices()?;
        if devices.iter().any(|device| device.id == target_device_id) {
            Ok(target_device_id)
        } else {
            Err(AppError::audio(format!(
                "selected virtual microphone input device is unavailable: {target_device_id}"
            )))
        }
    }
}

impl VirtualMicAdapter for SelectableVirtualMicAdapter {
    fn is_available(&self) -> bool {
        self.validate_target_device().is_ok()
    }

    fn start(&self, format: PcmFormat) -> AppResult<()> {
        format.validate().map_err(AppError::audio)?;
        self.validate_target_device()?;
        *self.running_format.write().expect("virtual mic lock poisoned") = Some(format);
        *self.accepted_frame_count.write().expect("virtual mic lock poisoned") = 0;
        Ok(())
    }

    fn write_frame(&self, frame: &AudioFrame) -> AppResult<()> {
        let expected_format = *self.running_format.read().expect("virtual mic lock poisoned");
        let Some(expected_format) = expected_format else {
            return Err(AppError::audio("virtual microphone is not running"));
        };
        if frame.format != expected_format {
            return Err(AppError::audio("virtual microphone frame format changed while running"));
        }

        *self.accepted_frame_count.write().expect("virtual mic lock poisoned") += 1;
        Ok(())
    }

    fn stop(&self) -> AppResult<()> {
        *self.running_format.write().expect("virtual mic lock poisoned") = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{SelectableVirtualMicAdapter, VirtualMicAdapter};
    use crate::audio::frame::{AudioFrame, PcmFormat};

    #[test]
    fn selectable_virtual_mic_requires_selected_input_device() {
        let adapter = SelectableVirtualMicAdapter::default();

        let error = adapter.start(PcmFormat::default()).unwrap_err().to_string();

        assert!(error.contains("virtualMicDeviceId"));
    }

    #[test]
    fn selectable_virtual_mic_rejects_unknown_input_device() {
        let adapter = SelectableVirtualMicAdapter::default();
        adapter.set_target_device_id(Some("missing-input-device".into()));

        let error = adapter.start(PcmFormat::default()).unwrap_err().to_string();

        assert!(error.contains("unavailable"));
    }

    #[test]
    fn selectable_virtual_mic_rejects_frames_when_stopped() {
        let adapter = SelectableVirtualMicAdapter::default();
        let format = PcmFormat::default();
        let frame = AudioFrame {
            sequence: 1,
            timestamp_ms: Utc::now().timestamp_millis(),
            format,
            samples: vec![0.0; format.samples_per_frame()],
        };

        let error = adapter.write_frame(&frame).unwrap_err().to_string();

        assert!(error.contains("not running"));
    }
}
