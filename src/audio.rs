//! Procedural arcade sound effects mixed by Quasar's device-optional audio stack.
//! One sound is one Quasar procedural voice; Quasar owns voice limiting and mixing.
//! The recipes remain an independent synthesizer, not a cycle-accurate POKEY model.
use quasar_audio::{
    AudioEngine, BusId, EngineConfig, PlaybackConfig, ProceduralSource, SoundSource, VoicePolicy,
};
use std::{
    error::Error,
    sync::atomic::{AtomicBool, Ordering},
};

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
    noise: u32,
    noise_phase: f32,
    noise_value: f32,
}

impl Voice {
    fn new(sound: Sound) -> Self {
        let duration = match sound {
            Sound::Launch => 0.14,
            Sound::Explosion => 0.48,
            Sound::CityLost => 0.9,
            Sound::Bonus => 0.12,
            Sound::Empty => 0.055,
            Sound::GameOver => 1.4,
        };
        Self {
            sound,
            age: 0.0,
            duration,
            phase: 0.0,
            noise: 0,
            noise_phase: 0.0,
            noise_value: 0.0,
        }
    }

    fn finished(&self) -> bool {
        self.age >= self.duration
    }

    fn sample(&mut self, sample_rate: f32) -> f32 {
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
        let fraction = self.age / self.duration;
        let envelope = (1.0 - fraction).max(0.0).powi(2);
        let (frequency, noise_mix, gain) = match self.sound {
            Sound::Launch => (1800.0 - 1500.0 * fraction, 0.15, 0.13),
            Sound::Explosion => (90.0 - 60.0 * fraction, 0.85, 0.18),
            Sound::CityLost => (65.0 - 45.0 * fraction, 0.9, 0.22),
            Sound::Bonus => (800.0 + 500.0 * fraction, 0.0, 0.1),
            Sound::Empty => (120.0, 0.0, 0.08),
            Sound::GameOver => (160.0 - 120.0 * fraction, 0.5, 0.18),
        };
        self.phase = (self.phase + frequency * dt).fract();
        let square = if self.phase < 0.5 { -1.0 } else { 1.0 };
        let mixed = (square * (1.0 - noise_mix) + self.noise_value * noise_mix) * envelope * gain;
        self.age += dt;
        mixed.clamp(-0.85, 0.85)
    }
}

/// One Quasar procedural voice for a single sound effect.
pub struct SfxSource {
    voice: Voice,
    sample_rate: u32,
}

impl SfxSource {
    pub fn new(sound: Sound, sample_rate: u32) -> Self {
        Self {
            voice: Voice::new(sound),
            sample_rate,
        }
    }
}

impl ProceduralSource for SfxSource {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        2
    }

    fn next_frame(&mut self, output: &mut [f32]) -> bool {
        if self.voice.finished() {
            return false;
        }
        output.fill(self.voice.sample(self.sample_rate as f32));
        true
    }
}

/// Host audio through Quasar's CPAL backend, with silent degradation on failure.
pub struct Audio {
    engine: AudioEngine,
    sample_rate: u32,
    muted: AtomicBool,
}

impl Audio {
    pub fn open(device: &str, muted: bool) -> Result<Self, Box<dyn Error>> {
        let device_name = (device != "default").then_some(device);
        let engine = AudioEngine::open(
            device_name,
            EngineConfig {
                max_voices: 24,
                ..EngineConfig::default()
            },
        )?;
        let sample_rate = engine.diagnostics().sample_rate;
        if sample_rate == 0 {
            return Err("audio device reported an invalid sample rate".into());
        }
        engine.set_muted(muted)?;
        Ok(Self {
            engine,
            sample_rate,
            muted: AtomicBool::new(muted),
        })
    }

    pub fn play(&self, sound: Sound) {
        let config = PlaybackConfig {
            bus: BusId::MASTER,
            policy: VoicePolicy::StealOldest,
            ..PlaybackConfig::default()
        };
        let _ = self.engine.play(
            SoundSource::Procedural(Box::new(SfxSource::new(sound, self.sample_rate))),
            config,
        );
    }

    pub fn set_muted(&self, muted: bool) {
        if self.muted.load(Ordering::Relaxed) != muted && self.engine.set_muted(muted).is_ok() {
            self.muted.store(muted, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_is_bounded_and_decays_to_silence() {
        let mut source = SfxSource::new(Sound::Explosion, 48_000);
        let mut frame = [0.0_f32; 2];
        let mut peak = 0.0_f32;
        let mut frames = 0;
        while source.next_frame(&mut frame) {
            for sample in frame {
                assert!(sample.is_finite() && sample.abs() <= 0.85);
                peak = peak.max(sample.abs());
            }
            frames += 1;
        }
        assert!(frames > 0);
        assert!(peak > 0.05);
        assert!(!source.next_frame(&mut frame));
    }

    #[test]
    fn quasar_mixer_renders_procedural_effects() {
        let mut mixer = quasar_audio::Mixer::new(quasar_audio::MixerConfig::default()).unwrap();
        mixer
            .play(
                SoundSource::Procedural(Box::new(SfxSource::new(
                    Sound::Explosion,
                    mixer.sample_rate(),
                ))),
                PlaybackConfig::default(),
            )
            .unwrap();
        let pcm = mixer.render(4_800).unwrap();
        assert!(pcm.iter().any(|sample| sample.abs() > 0.01));
    }
}
