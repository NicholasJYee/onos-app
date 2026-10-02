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

// ---------------------------------------------------------------------------
// AVAudioSession
//
// Enumerating devices is not enough to record on iOS. Every app starts in the
// `soloAmbient` category, which makes audio *input* unavailable no matter what
// the microphone permission says -- the input unit starts but delivers silence.
// Recording requires explicitly setting a record-capable category and
// activating the session. cpal exposes no API for this, so it is done here
// through the Objective-C runtime. AVFoundation is already linked for iOS in
// build.rs.
// ---------------------------------------------------------------------------

use objc::runtime::{Object, BOOL, NO, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    /// `NSString *` global exported by AVFoundation.
    static AVAudioSessionCategoryPlayAndRecord: *const Object;
}

/// `AVAudioSessionCategoryOptions`. Allows a Bluetooth headset to be used as
/// the input, and routes playback to the speaker rather than the earpiece.
const OPTION_ALLOW_BLUETOOTH: usize = 0x4;
const OPTION_DEFAULT_TO_SPEAKER: usize = 0x8;

/// Reads an `NSString *` into a Rust `String`.
unsafe fn nsstring_to_string(s: *mut Object) -> String {
    if s.is_null() {
        return String::new();
    }
    let utf8: *const c_char = msg_send![s, UTF8String];
    if utf8.is_null() {
        return String::new();
    }
    CStr::from_ptr(utf8).to_string_lossy().into_owned()
}

/// Describes an `NSError *` for logging.
unsafe fn nserror_to_string(err: *mut Object) -> String {
    if err.is_null() {
        return "unknown error".to_string();
    }
    let desc: *mut Object = msg_send![err, localizedDescription];
    let s = nsstring_to_string(desc);
    if s.is_empty() {
        "unknown error".to_string()
    } else {
        s
    }
}

/// Puts the process into a record-capable audio session and activates it.
///
/// Call this before opening an input stream. Safe to call repeatedly; iOS
/// treats setting the same category and re-activating as a no-op.
pub fn activate_audio_session() -> Result<()> {
    unsafe {
        let session: *mut Object = msg_send![class!(AVAudioSession), sharedInstance];
        if session.is_null() {
            anyhow::bail!("AVAudioSession sharedInstance returned nil");
        }

        let mut err: *mut Object = ptr::null_mut();
        let ok: BOOL = msg_send![
            session,
            setCategory: AVAudioSessionCategoryPlayAndRecord
            withOptions: (OPTION_ALLOW_BLUETOOTH | OPTION_DEFAULT_TO_SPEAKER)
            error: &mut err
        ];
        if ok == NO {
            anyhow::bail!(
                "failed to set AVAudioSession category: {}",
                nserror_to_string(err)
            );
        }

        // The pipeline assumes 48 kHz; ask for it rather than accept whatever
        // the current route offers. This is only a preference -- iOS may ignore
        // it, and capture still resamples, so a failure here is not fatal.
        let mut rate_err: *mut Object = ptr::null_mut();
        let rate_ok: BOOL = msg_send![
            session,
            setPreferredSampleRate: 48000.0f64
            error: &mut rate_err
        ];
        if rate_ok == NO {
            log::warn!(
                "could not request a 48 kHz sample rate: {}",
                nserror_to_string(rate_err)
            );
        }

        let mut activate_err: *mut Object = ptr::null_mut();
        let activated: BOOL = msg_send![session, setActive: YES error: &mut activate_err];
        if activated == NO {
            anyhow::bail!(
                "failed to activate AVAudioSession: {}",
                nserror_to_string(activate_err)
            );
        }

        let actual_rate: f64 = msg_send![session, sampleRate];
        log::info!("AVAudioSession active (playAndRecord) at {} Hz", actual_rate);
    }

    Ok(())
}

/// Releases the audio session so other apps regain control of audio.
///
/// Failure is logged rather than propagated: by the time this runs the
/// recording has already stopped, and there is nothing useful to do about it.
pub fn deactivate_audio_session() {
    unsafe {
        let session: *mut Object = msg_send![class!(AVAudioSession), sharedInstance];
        if session.is_null() {
            return;
        }
        let mut err: *mut Object = ptr::null_mut();
        let ok: BOOL = msg_send![session, setActive: NO error: &mut err];
        if ok == NO {
            log::warn!(
                "failed to deactivate AVAudioSession: {}",
                nserror_to_string(err)
            );
        } else {
            log::info!("AVAudioSession deactivated");
        }
    }
}
