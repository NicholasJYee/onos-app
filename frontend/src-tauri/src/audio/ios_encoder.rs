//! AAC/m4a audio writing for iOS, via AVFoundation.
//!
//! Every other platform encodes audio by shelling out to the bundled ffmpeg
//! sidecar (see `audio::encode::encode_single_audio`). iOS forbids
//! subprocesses and bundles no such binary, so `find_ffmpeg_path()` returns
//! `None` there and every checkpoint fails with "FFmpeg not found" -- which is
//! why recordings produced no audio file at all on the phone.
//!
//! `AVAudioFile` covers it without a subprocess: samples are handed to it as
//! they arrive and it encodes to AAC in an .m4a container, which the Files app
//! can play in place.
//!
//! Only mono is supported, which is all the pipeline produces: it mixes
//! microphone and system audio down to a single channel before saving.

use anyhow::{anyhow, Result};
use objc::runtime::{Object, BOOL, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    /// `NSString *` keys for the AVAudioFile settings dictionary.
    static AVFormatIDKey: *const Object;
    static AVSampleRateKey: *const Object;
    static AVNumberOfChannelsKey: *const Object;
    static AVEncoderAudioQualityKey: *const Object;
}

/// `kAudioFormatMPEG4AAC`, the four-character code `'aac '`.
const K_AUDIO_FORMAT_MPEG4_AAC: u32 = 0x6161_6320;

/// `AVAudioQualityHigh`.
const AV_AUDIO_QUALITY_HIGH: i64 = 0x60;

unsafe fn nsstring(value: &str) -> Result<*mut Object> {
    let c_value = CString::new(value).map_err(|_| anyhow!("path contains an interior nul byte"))?;
    let string: *mut Object = msg_send![class!(NSString), stringWithUTF8String: c_value.as_ptr()];
    if string.is_null() {
        return Err(anyhow!("failed to create NSString"));
    }
    Ok(string)
}

unsafe fn nsnumber_u32(value: u32) -> *mut Object {
    msg_send![class!(NSNumber), numberWithUnsignedInt: value]
}

unsafe fn nsnumber_f64(value: f64) -> *mut Object {
    msg_send![class!(NSNumber), numberWithDouble: value]
}

unsafe fn nsnumber_i64(value: i64) -> *mut Object {
    msg_send![class!(NSNumber), numberWithLongLong: value]
}

/// Describes an `NSError *` for logging.
unsafe fn nserror_to_string(err: *mut Object) -> String {
    if err.is_null() {
        return "unknown error".to_string();
    }
    let description: *mut Object = msg_send![err, localizedDescription];
    if description.is_null() {
        return "unknown error".to_string();
    }
    let utf8: *const c_char = msg_send![description, UTF8String];
    if utf8.is_null() {
        return "unknown error".to_string();
    }
    CStr::from_ptr(utf8).to_string_lossy().into_owned()
}

/// Streams mono f32 samples into an AAC .m4a file.
pub struct IosAudioWriter {
    /// `AVAudioFile *`, owned. Releasing it flushes and closes the file.
    file: *mut Object,
    /// `AVAudioFormat *` (the file's processing format), retained.
    format: *mut Object,
    channels: u32,
}

// SAFETY: the Objective-C objects held here are only ever touched through
// `&mut self`, so they are never used from two threads at once.
unsafe impl Send for IosAudioWriter {}

impl IosAudioWriter {
    /// Opens `path` for writing. The file is finished when the writer is dropped.
    pub fn create(path: &Path, sample_rate: u32, channels: u16) -> Result<Self> {
        if channels != 1 {
            return Err(anyhow!(
                "iOS audio writer only supports mono, got {} channels",
                channels
            ));
        }

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow!("recording path is not valid UTF-8: {}", path.display()))?;

        unsafe {
            let ns_path = nsstring(path_str)?;
            let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: ns_path];
            if url.is_null() {
                return Err(anyhow!("failed to build a file URL for {}", path_str));
            }

            let settings: *mut Object = msg_send![class!(NSMutableDictionary), dictionary];
            let _: () = msg_send![settings,
                setObject: nsnumber_u32(K_AUDIO_FORMAT_MPEG4_AAC)
                forKey: AVFormatIDKey];
            let _: () = msg_send![settings,
                setObject: nsnumber_f64(sample_rate as f64)
                forKey: AVSampleRateKey];
            let _: () = msg_send![settings,
                setObject: nsnumber_u32(channels as u32)
                forKey: AVNumberOfChannelsKey];
            let _: () = msg_send![settings,
                setObject: nsnumber_i64(AV_AUDIO_QUALITY_HIGH)
                forKey: AVEncoderAudioQualityKey];

            let mut err: *mut Object = ptr::null_mut();
            let file: *mut Object = msg_send![class!(AVAudioFile), alloc];
            let file: *mut Object = msg_send![file,
                initForWriting: url
                settings: settings
                error: &mut err];
            if file.is_null() {
                return Err(anyhow!(
                    "AVAudioFile could not open {} for writing: {}",
                    path_str,
                    nserror_to_string(err)
                ));
            }

            // Buffers handed to writeFromBuffer: must be in the file's
            // processing format, which for a file opened this way is
            // deinterleaved float32 at the requested sample rate.
            let format: *mut Object = msg_send![file, processingFormat];
            if format.is_null() {
                let _: () = msg_send![file, release];
                return Err(anyhow!("AVAudioFile reported no processing format"));
            }
            let format: *mut Object = msg_send![format, retain];
            let channels: u32 = msg_send![format, channelCount];

            log::info!(
                "Writing AAC audio to {} ({} Hz, {} ch)",
                path_str,
                sample_rate,
                channels
            );

            Ok(Self {
                file,
                format,
                channels,
            })
        }
    }

    /// Encodes and appends `samples` (mono, f32).
    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        if self.channels != 1 {
            return Err(anyhow!(
                "expected a mono processing format, got {} channels",
                self.channels
            ));
        }

        let frames = samples.len() as u32;

        unsafe {
            let buffer: *mut Object = msg_send![class!(AVAudioPCMBuffer), alloc];
            let buffer: *mut Object = msg_send![buffer,
                initWithPCMFormat: self.format
                frameCapacity: frames];
            if buffer.is_null() {
                return Err(anyhow!("could not allocate an AVAudioPCMBuffer"));
            }

            let _: () = msg_send![buffer, setFrameLength: frames];

            // `float * const *`: one pointer per channel. Mono, so channel 0.
            let channel_data: *const *mut f32 = msg_send![buffer, floatChannelData];
            if channel_data.is_null() {
                let _: () = msg_send![buffer, release];
                return Err(anyhow!("AVAudioPCMBuffer exposed no float channel data"));
            }
            ptr::copy_nonoverlapping(samples.as_ptr(), *channel_data, samples.len());

            let mut err: *mut Object = ptr::null_mut();
            let ok: BOOL = msg_send![self.file, writeFromBuffer: buffer error: &mut err];
            let _: () = msg_send![buffer, release];

            if ok == NO {
                return Err(anyhow!("failed to write audio: {}", nserror_to_string(err)));
            }
        }

        Ok(())
    }
}

impl Drop for IosAudioWriter {
    fn drop(&mut self) {
        unsafe {
            // Releasing the AVAudioFile is what finishes the container: until it
            // is deallocated the .m4a has no moov atom and will not play.
            if !self.file.is_null() {
                let _: () = msg_send![self.file, release];
                self.file = ptr::null_mut();
            }
            if !self.format.is_null() {
                let _: () = msg_send![self.format, release];
                self.format = ptr::null_mut();
            }
        }
    }
}
