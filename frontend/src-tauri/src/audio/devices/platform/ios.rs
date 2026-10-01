use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};

use crate::audio::devices::configuration::{AudioDevice, DeviceType};

/// Configure iOS audio devices.
///
/// iOS exposes a single RemoteIO audio unit rather than a list of selectable
/// devices: whichever input the system has routed (built-in mic, headset,
/// Bluetooth) is presented as the default. There is also no equivalent of
/// ScreenCaptureKit, so no output device can be captured — recordings on iOS
/// are microphone-only.
pub fn configure_ios_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices: Vec<AudioDevice> = Vec::new();

    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            devices.push(AudioDevice::new(name, DeviceType::Input));
        }
    }

    // Deliberately no output devices: iOS cannot capture system audio.
    Ok(devices)
}
