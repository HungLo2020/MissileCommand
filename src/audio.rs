//! Procedural arcade sound effects. Linux playback uses the host's libasound.
//! This is an independent synthesizer, not a cycle-accurate POKEY implementation.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug)]
pub enum Sound {
    Launch,
    Explosion,
    CityLost,
    Bonus,
    Empty,
    GameOver,
}

struct Voice {
    sound: Sound,
    age: f32,
    duration: f32,
    phase: f32,
}

#[derive(Default)]
pub struct Synth {
    voices: Vec<Voice>,
    noise: u32,
    noise_phase: f32,
    noise_value: f32,
}

impl Synth {
    pub fn trigger(&mut self, sound: Sound) {
        if self.voices.len() == 24 {
            self.voices.remove(0);
        }
        let duration = match sound {
            Sound::Launch => 0.14,
            Sound::Explosion => 0.48,
            Sound::CityLost => 0.9,
            Sound::Bonus => 0.12,
            Sound::Empty => 0.055,
            Sound::GameOver => 1.4,
        };
        self.voices.push(Voice {
            sound,
            age: 0.0,
            duration,
            phase: 0.0,
        });
    }

    pub fn clear(&mut self) {
        self.voices.clear();
    }

    pub fn sample(&mut self, sample_rate: f32) -> f32 {
        let dt = 1.0 / sample_rate;
        self.noise_phase += 5000.0 * dt;
        if self.noise_phase >= 1.0 {
            self.noise_phase -= 1.0;
            if self.noise == 0 {
                self.noise = 0x1ffff;
            }
            let feedback = ((self.noise >> 3) ^ self.noise) & 1;
            self.noise = (self.noise >> 1) | (feedback << 16);
            self.noise_value = if self.noise & 1 == 0 { -1.0 } else { 1.0 };
        }
        let mut mixed = 0.0;
        for voice in &mut self.voices {
            let fraction = voice.age / voice.duration;
            let envelope = (1.0 - fraction).max(0.0).powi(2);
            let (frequency, noise_mix, gain) = match voice.sound {
                Sound::Launch => (1800.0 - 1500.0 * fraction, 0.15, 0.13),
                Sound::Explosion => (90.0 - 60.0 * fraction, 0.85, 0.18),
                Sound::CityLost => (65.0 - 45.0 * fraction, 0.9, 0.22),
                Sound::Bonus => (800.0 + 500.0 * fraction, 0.0, 0.1),
                Sound::Empty => (120.0, 0.0, 0.08),
                Sound::GameOver => (160.0 - 120.0 * fraction, 0.5, 0.18),
            };
            voice.phase = (voice.phase + frequency * dt).fract();
            let square = if voice.phase < 0.5 { -1.0 } else { 1.0 };
            mixed += (square * (1.0 - noise_mix) + self.noise_value * noise_mix) * envelope * gain;
            voice.age += dt;
        }
        self.voices.retain(|v| v.age < v.duration);
        mixed.clamp(-0.85, 0.85)
    }
}

pub struct Audio {
    sender: SyncSender<Sound>,
    muted: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Audio {
    #[cfg(target_os = "linux")]
    pub fn open(device: &str, muted: bool) -> Result<Self, Box<dyn std::error::Error>> {
        use alsa::{
            Direction, ValueOr,
            pcm::{Access, Format, HwParams, PCM},
        };
        let pcm = PCM::new(device, Direction::Playback, false)?;
        let rate;
        {
            let params = HwParams::any(&pcm)?;
            params.set_channels(2)?;
            params.set_rate(48_000, ValueOr::Nearest)?;
            params.set_access(Access::RWInterleaved)?;
            params.set_format(Format::s16())?;
            params.set_buffer_time_near(40_000, ValueOr::Nearest)?;
            params.set_period_time_near(5_000, ValueOr::Nearest)?;
            pcm.hw_params(&params)?;
            rate = params.get_rate()?;
        }
        let (sender, receiver) = mpsc::sync_channel(64);
        let muted = Arc::new(AtomicBool::new(muted));
        let running = Arc::new(AtomicBool::new(true));
        let mute_flag = muted.clone();
        let run_flag = running.clone();
        let worker = std::thread::Builder::new()
            .name("alsa-playback".into())
            .spawn(move || {
                let Ok(output) = pcm.io_i16() else {
                    return;
                };
                let mut synth = Synth::default();
                let mut samples = [0_i16; 512];
                while run_flag.load(Ordering::Relaxed) {
                    for sound in receiver.try_iter().take(64) {
                        synth.trigger(sound);
                    }
                    if mute_flag.load(Ordering::Relaxed) {
                        synth.clear();
                    }
                    for frame in samples.chunks_exact_mut(2) {
                        let sample = (synth.sample(rate as f32) * i16::MAX as f32) as i16;
                        frame.fill(sample);
                    }
                    let mut offset = 0;
                    while offset < samples.len() && run_flag.load(Ordering::Relaxed) {
                        match output.writei(&samples[offset..]) {
                            Ok(0) => {
                                std::thread::sleep(std::time::Duration::from_millis(1));
                            }
                            Ok(frames) => offset += frames * 2,
                            Err(error) => {
                                if pcm.try_recover(error, true).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                let _ = pcm.drop();
            })?;
        Ok(Self {
            sender,
            muted,
            running,
            worker: Some(worker),
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open(_device: &str, _muted: bool) -> Result<Self, Box<dyn std::error::Error>> {
        Err("ALSA playback is available on Linux".into())
    }

    pub fn play(&self, sound: Sound) {
        let _ = self.sender.try_send(sound);
    }
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sound_is_bounded_and_decays_to_silence() {
        let mut synth = Synth::default();
        synth.trigger(Sound::Explosion);
        let mut peak = 0.0_f32;
        for _ in 0..48_000 {
            let sample = synth.sample(48_000.0);
            assert!(sample.is_finite() && sample.abs() <= 0.85);
            peak = peak.max(sample.abs());
        }
        assert!(peak > 0.05);
        assert_eq!(synth.sample(48_000.0), 0.0);
    }
}
