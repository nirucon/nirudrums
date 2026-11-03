use crate::project::{ReverbMode, ReverbSettings};

pub fn db_to_gain(db: f32) -> f32 {
    if db <= -96.0 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

pub fn soft_clip(input: f32, drive: f32) -> f32 {
    if drive <= 0.0001 {
        return input;
    }
    let gain = 1.0 + drive * 6.0;
    (input * gain).tanh() / (1.0 + drive * 1.5)
}

#[derive(Clone, Debug)]
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    delay: usize,
    feedback: f32,
    damp: f32,
    filter: f32,
}

impl Comb {
    fn new(max_delay: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay.max(1)],
            index: 0,
            delay: max_delay.max(1),
            feedback: 0.7,
            damp: 0.4,
            filter: 0.0,
        }
    }

    fn set(&mut self, delay: usize, feedback: f32, damp: f32) {
        self.delay = delay.clamp(1, self.buffer.len());
        self.index %= self.delay;
        self.feedback = feedback.clamp(0.0, 0.97);
        self.damp = damp.clamp(0.0, 0.95);
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter = output * (1.0 - self.damp) + self.filter * self.damp;
        self.buffer[self.index] = input + self.filter * self.feedback;
        self.index += 1;
        if self.index >= self.delay {
            self.index = 0;
        }
        output
    }
}

#[derive(Clone, Debug)]
struct AllPass {
    buffer: Vec<f32>,
    index: usize,
    delay: usize,
    feedback: f32,
}

impl AllPass {
    fn new(max_delay: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay.max(1)],
            index: 0,
            delay: max_delay.max(1),
            feedback: 0.5,
        }
    }

    fn set(&mut self, delay: usize) {
        self.delay = delay.clamp(1, self.buffer.len());
        self.index %= self.delay;
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.index];
        let output = -input + delayed;
        self.buffer[self.index] = input + delayed * self.feedback;
        self.index += 1;
        if self.index >= self.delay {
            self.index = 0;
        }
        output
    }
}

#[derive(Clone, Debug)]
pub struct Reverb {
    sample_rate: f32,
    pre: Vec<f32>,
    pre_index: usize,
    pre_delay: usize,
    comb_l: [Comb; 4],
    comb_r: [Comb; 4],
    allpass_l: [AllPass; 2],
    allpass_r: [AllPass; 2],
    gate_env: f32,
    gate_coef: f32,
    mode: ReverbMode,
}

impl Default for Reverb {
    fn default() -> Self {
        Self::new()
    }
}

impl Reverb {
    pub fn new() -> Self {
        let max_comb = 8192;
        let max_allpass = 2048;
        Self {
            sample_rate: 48_000.0,
            pre: vec![0.0; 16_384],
            pre_index: 0,
            pre_delay: 1,
            comb_l: std::array::from_fn(|_| Comb::new(max_comb)),
            comb_r: std::array::from_fn(|_| Comb::new(max_comb)),
            allpass_l: std::array::from_fn(|_| AllPass::new(max_allpass)),
            allpass_r: std::array::from_fn(|_| AllPass::new(max_allpass)),
            gate_env: 0.0,
            gate_coef: 0.0,
            mode: ReverbMode::Plate,
        }
    }

    pub fn prepare(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        let max_comb = (self.sample_rate * 0.08).ceil() as usize + 2;
        let max_allpass = (self.sample_rate * 0.015).ceil() as usize + 2;
        let pre = (self.sample_rate * 0.2).ceil() as usize + 2;
        self.pre = vec![0.0; pre];
        self.pre_index = 0;
        for comb in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            *comb = Comb::new(max_comb);
        }
        for allpass in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            *allpass = AllPass::new(max_allpass);
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        self.pre.fill(0.0);
        self.pre_index = 0;
        for comb in self.comb_l.iter_mut().chain(self.comb_r.iter_mut()) {
            comb.buffer.fill(0.0);
            comb.index = 0;
            comb.filter = 0.0;
        }
        for allpass in self.allpass_l.iter_mut().chain(self.allpass_r.iter_mut()) {
            allpass.buffer.fill(0.0);
            allpass.index = 0;
        }
        self.gate_env = 0.0;
    }

    pub fn set_settings(&mut self, settings: &ReverbSettings) {
        self.mode = settings.mode;
        let scale = self.sample_rate / 44_100.0;
        let base: [f64; 4] = match settings.mode {
            ReverbMode::Plate => [0.0297, 0.0371, 0.0411, 0.0437],
            ReverbMode::Room => [0.0313, 0.0379, 0.0419, 0.0473],
            ReverbMode::Gated => [0.0271, 0.0337, 0.0391, 0.0449],
        };
        let feedback = match settings.mode {
            ReverbMode::Plate => 0.74 + settings.decay.clamp(0.0, 1.0) * 0.20,
            ReverbMode::Room => 0.70 + settings.decay.clamp(0.0, 1.0) * 0.24,
            ReverbMode::Gated => 0.80 + settings.decay.clamp(0.0, 1.0) * 0.16,
        };
        let damping = settings.damping.clamp(0.0, 0.95);
        for (index, comb) in self.comb_l.iter_mut().enumerate() {
            comb.set(
                (base[index] * self.sample_rate as f64).round() as usize,
                feedback,
                damping,
            );
        }
        for (index, comb) in self.comb_r.iter_mut().enumerate() {
            comb.set(
                ((base[index] + 0.0007 + index as f64 * 0.00013) * self.sample_rate as f64).round()
                    as usize,
                feedback,
                damping,
            );
        }
        self.allpass_l[0].set((0.0050 * scale).round() as usize);
        self.allpass_l[1].set((0.0017 * scale).round() as usize);
        self.allpass_r[0].set((0.0057 * scale).round() as usize);
        self.allpass_r[1].set((0.0021 * scale).round() as usize);
        self.pre_delay = ((settings.pre_delay_ms.clamp(0.0, 200.0) / 1000.0) * self.sample_rate)
            .round() as usize;
        self.pre_delay = self.pre_delay.clamp(1, self.pre.len());
        let gate_seconds = (settings.gate_ms.clamp(0.0, 2000.0) / 1000.0).max(0.001);
        self.gate_coef = (-1.0 / (gate_seconds * self.sample_rate)).exp();
    }

    pub fn process(&mut self, left: f32, right: f32, settings: &ReverbSettings) -> (f32, f32) {
        let mono = (left + right) * 0.5;
        self.pre[self.pre_index] = mono;
        let read = if self.pre_index >= self.pre_delay {
            self.pre_index - self.pre_delay
        } else {
            self.pre_index + self.pre.len() - self.pre_delay
        };
        let delayed = self.pre[read];
        self.pre_index += 1;
        if self.pre_index >= self.pre.len() {
            self.pre_index = 0;
        }

        let mut wet_l = 0.0;
        let mut wet_r = 0.0;
        for comb in &mut self.comb_l {
            wet_l += comb.process(delayed);
        }
        for comb in &mut self.comb_r {
            wet_r += comb.process(delayed);
        }
        wet_l *= 0.25;
        wet_r *= 0.25;
        for allpass in &mut self.allpass_l {
            wet_l = allpass.process(wet_l);
        }
        for allpass in &mut self.allpass_r {
            wet_r = allpass.process(wet_r);
        }

        if self.mode == ReverbMode::Gated {
            let activity = (left.abs() + right.abs()).min(1.0);
            self.gate_env = (self.gate_env + activity * 0.08).min(1.0);
            if activity < 0.0001 {
                self.gate_env *= self.gate_coef;
            }
            wet_l *= self.gate_env;
            wet_r *= self.gate_env;
        } else {
            self.gate_env = 1.0;
        }

        let width = settings.width.clamp(0.0, 1.0);
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5 * width;
        wet_l = mid + side;
        wet_r = mid - side;
        let mix = settings.mix.clamp(0.0, 1.0);
        (left + wet_l * mix, right + wet_r * mix)
    }
}

#[derive(Clone, Debug)]
pub struct Limiter {
    env: f32,
    attack: f32,
    release: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        let mut limiter = Self {
            env: 0.0,
            attack: 0.0,
            release: 0.0,
        };
        limiter.prepare(sample_rate);
        limiter
    }

    pub fn prepare(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(1.0);
        self.attack = (-1.0 / (0.001 * sr)).exp();
        self.release = (-1.0 / (0.100 * sr)).exp();
    }

    pub fn process(&mut self, input: f32, threshold: f32) -> f32 {
        let magnitude = input.abs();
        if magnitude > self.env {
            self.env = magnitude;
        } else {
            self.env = self.release * self.env + (1.0 - self.release) * magnitude;
        }
        if self.env > threshold && threshold > 0.0 {
            input * (threshold / self.env)
        } else {
            input
        }
    }
}

pub fn default_reverb_settings() -> ReverbSettings {
    ReverbSettings::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_conversion_is_sane() {
        assert!((db_to_gain(0.0) - 1.0).abs() < 1e-6);
        assert!(db_to_gain(-96.0) == 0.0);
    }

    #[test]
    fn soft_clip_stays_finite_and_bounded() {
        for sample in [-4.0, -1.0, 0.0, 0.5, 4.0] {
            let output = soft_clip(sample, 0.7);
            assert!(output.is_finite());
            assert!(output.abs() <= 1.5);
        }
    }

    #[test]
    fn reverb_impulse_decays_to_finite_tail() {
        let mut reverb = Reverb::new();
        reverb.prepare(48_000.0);
        reverb.set_settings(&ReverbSettings::default());
        let mut last = 0.0;
        for _ in 0..48_000 {
            let (l, r) = reverb.process(0.0, 0.0, &ReverbSettings::default());
            assert!(l.is_finite() && r.is_finite());
            last = l.abs().max(r.abs());
        }
        assert!(last < 0.5);
    }

    #[test]
    fn limiter_limits_hot_signal() {
        let mut limiter = Limiter::new(48_000.0);
        for _ in 0..1_000 {
            let _ = limiter.process(4.0, 0.95);
        }
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            peak = peak.max(limiter.process(4.0, 0.95).abs());
        }
        assert!(peak < 1.1);
    }
}
