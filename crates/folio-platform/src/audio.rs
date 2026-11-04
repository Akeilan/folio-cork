//! Sound output (X3-13), behind the `audio` feature so apps without sound don't depend on the
//! system's sound libraries. The app fills interleaved f32 samples; the platform plays them.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// A running sound output. Dropping it stops the sound.
pub struct AudioOut {
    _stream: cpal::Stream,
    pub rate: u32,
    pub channels: u16,
}

/// Open the default sound output. `fill` gets interleaved samples (-1.0..1.0) to write, from
/// the sound thread. Fails with a plain message when there's no sound device.
pub fn audio_out<F: FnMut(&mut [f32]) + Send + 'static>(rate: u32, channels: u16, mut fill: F) -> Result<AudioOut, String> {
    let host = cpal::default_host();
    let dev = host.default_output_device().ok_or("There's no sound output on this computer.")?;
    let cfg = cpal::StreamConfig { channels, sample_rate: rate, buffer_size: cpal::BufferSize::Default };
    let stream = dev
        .build_output_stream(cfg, move |data: &mut [f32], _| fill(data), |e| eprintln!("sound: {e}"), None)
        .map_err(|e| format!("The sound output can't play {rate} Hz, {channels} channels: {e}"))?;
    stream.play().map_err(|e| format!("The sound output won't start: {e}"))?;
    Ok(AudioOut { _stream: stream, rate, channels })
}

/// Whether the computer has a sound output at all.
pub fn has_audio_out() -> bool {
    cpal::default_host().default_output_device().is_some()
}
