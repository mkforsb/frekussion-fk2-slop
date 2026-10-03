//! Panel state, the controller shared through the Dioxus context, and
//! persistence (localStorage on the web, `~/.config/frekussion` on desktop).

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use frekussion_dsp::params::{ALGORITHMS, OpParam, ParamKind, default_patch};
use frekussion_dsp::presets::Kit;
use frekussion_dsp::voice::Noise;
use frekussion_dsp::{ALL_PARAMS, OPS, Param, VOICES, VoicePatch};

use crate::audio::{AudioHandle, Command, ParamCache};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelState {
    pub voices: [VoicePatch; VOICES],
    pub master: f32,
    /// Per-channel MUTATE knob: 0 = off … 1 = full randomization on every
    /// trigger. A performance control rather than part of the sound, so it
    /// lives outside the patch and survives presets, RND and channel copies.
    pub mutate: [f32; VOICES],
}

impl Default for PanelState {
    fn default() -> Self {
        // Start with a kick on one channel and a snare on the other.
        let voices = frekussion_dsp::presets::KITS
            .first()
            .map_or([default_patch(); VOICES], Kit::patches);
        Self {
            voices,
            master: frekussion_dsp::engine::DEFAULT_MASTER,
            mutate: [0.0; VOICES],
        }
    }
}

/// Controls RND and MUTATE never change: the channel's level, trigger
/// sensitivity, stereo placement and whether it is linked to the other one.
fn keeps_value_on_randomize(p: Param) -> bool {
    matches!(p, Param::Level | Param::Sense | Param::Pan | Param::Link)
}

/// A random patch, as produced by the RND button. `current` supplies the
/// values of the controls that RND leaves alone.
///
/// Ranges are shaped so that the result is almost always a playable hit:
/// every carrier of the chosen algorithm is audible, decays stay short to
/// medium, and the effects that can dominate a sound (noise, drive, filter,
/// sweep, cross modulation) are often left out.
pub fn random_patch(current: &VoicePatch, rng: &mut Noise) -> VoicePatch {
    let mut patch = *current;
    let mut uniform = || rng.sample() * 0.5 + 0.5;
    // Draw every value in a fixed order so a seed always gives one patch.
    let draws: Vec<(f32, f32)> = ALL_PARAMS.iter().map(|_| (uniform(), uniform())).collect();
    let algo = ALGORITHMS[(draws[Param::Algorithm.index()].0 * ALGORITHMS.len() as f32)
        .floor()
        .min(ALGORITHMS.len() as f32 - 1.0) as usize];
    for p in ALL_PARAMS {
        if keeps_value_on_randomize(p) {
            continue;
        }
        let (x, coin) = draws[p.index()];
        let sometimes = |value: f32| if coin < 0.5 { value } else { p.default_value() };
        let op_role = (0..OPS).find_map(|op| {
            OpParam::ALL
                .iter()
                .find(|w| w.of(op) == p)
                .map(|&w| (w, algo.is_carrier(op)))
        });
        patch[p.index()] = p.sanitize(match (p, p.info().kind, op_role) {
            (_, ParamKind::Choice(names), _) => (x * names.len() as f32).floor(),
            (_, _, Some((OpParam::Level, true))) => 0.5 + 0.45 * x,
            (_, _, Some((OpParam::Level, false))) => 0.75 * x,
            (_, _, Some((OpParam::Decay, _))) => 0.15 + 0.65 * x,
            // Mostly in tune, sometimes detuned.
            (_, _, Some((OpParam::Fine, _))) => sometimes(0.25 + 0.5 * x),
            (_, _, Some((OpParam::Ratio, _))) => x,
            (Param::Tune, _, _) => 0.08 + 0.84 * x,
            (Param::Decay, _, _) => 0.15 + 0.55 * x,
            (Param::Feedback, _, _) => 0.7 * x * x,
            (Param::SweepAmount, _, _) => sometimes(0.15 + 0.8 * x),
            (Param::Drive, _, _) => sometimes(0.6 * x),
            (Param::Filter, _, _) => sometimes(0.2 + 0.6 * x),
            (Param::Resonance | Param::NoiseRes, _, _) => 0.6 * x,
            (Param::NoiseLevel, _, _) => sometimes(0.3 + 0.6 * x),
            (Param::NoiseDecay, _, _) => 0.1 + 0.6 * x,
            (Param::NoiseFm, _, _) => sometimes(0.6 * x),
            (Param::LfoDepth, _, _) => sometimes(0.6 * x * x),
            (Param::XmodAmount, _, _) => sometimes(0.7 * x),
            (Param::Ring, _, _) => sometimes(0.5 * x),
            // Often dry, sometimes drenched.
            (Param::ReverbMix, _, _) => 0.75 * x * x,
            _ => x,
        });
    }
    patch
}

/// Move every randomizable control `amount` (0..=1) of the way towards a fresh
/// random patch: `clamp(current + (random - current) * amount)`. At 1 this is
/// exactly [`random_patch`]; choice controls round to the nearest position.
pub fn mutate_patch(current: &VoicePatch, amount: f32, rng: &mut Noise) -> VoicePatch {
    let target = random_patch(current, rng);
    if amount >= 1.0 {
        return target;
    }
    let amount = amount.max(0.0);
    let mut patch = *current;
    for p in ALL_PARAMS {
        let (c, t) = (current[p.index()], target[p.index()]);
        patch[p.index()] = p.sanitize(c + (t - c) * amount);
    }
    patch
}

pub fn mutate_display(amount: f32) -> String {
    if amount <= 0.0 {
        "OFF".into()
    } else if amount >= 1.0 {
        "RND".into()
    } else {
        format!("{:.0}%", amount * 100.0)
    }
}

const FORMAT_HEADER: &str = "frekussion-state 1";

impl PanelState {
    pub fn to_cache(self) -> ParamCache {
        ParamCache {
            voices: self.voices,
            master: self.master,
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = format!("{FORMAT_HEADER}\nmaster {}\n", self.master);
        for (v, m) in self.mutate.iter().enumerate() {
            out += &format!("mutate {v} {m}\n");
        }
        for (v, patch) in self.voices.iter().enumerate() {
            for p in ALL_PARAMS {
                out += &format!("{v} {p:?} {}\n", patch[p.index()]);
            }
        }
        out
    }

    /// Lenient: unknown lines are skipped and missing values keep defaults.
    pub fn deserialize(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()?.trim() != FORMAT_HEADER {
            return None;
        }
        let mut state = PanelState::default();
        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["master", v] => {
                    if let Ok(v) = v.parse::<f32>() {
                        state.master = v.clamp(0.0, 1.0);
                    }
                }
                ["mutate", voice, v] => {
                    if let (Ok(voice), Ok(v)) = (voice.parse::<usize>(), v.parse::<f32>())
                        && let Some(m) = state.mutate.get_mut(voice)
                        && v.is_finite()
                    {
                        *m = v.clamp(0.0, 1.0);
                    }
                }
                [voice, name, v] => {
                    let (Ok(voice), Ok(v)) = (voice.parse::<usize>(), v.parse::<f32>()) else {
                        continue;
                    };
                    let param = ALL_PARAMS.iter().find(|p| format!("{p:?}") == *name);
                    if let (Some(patch), Some(p)) = (state.voices.get_mut(voice), param) {
                        patch[p.index()] = p.sanitize(v);
                    }
                }
                _ => {}
            }
        }
        Some(state)
    }
}

/// What a drag gesture is adjusting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Param(usize, Param),
    Master,
    /// A channel's MUTATE knob.
    Mutate(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub target: Target,
    pub start_y: f64,
    pub start_value: f32,
    /// Pixels of vertical travel for the full range.
    pub span_px: f64,
    pub fine: bool,
}

/// Everything the controls need, shared via `use_context::<Synth>()`.
#[derive(Clone)]
pub struct Synth {
    pub state: Signal<PanelState>,
    pub audio: AudioHandle,
    /// Last touched parameter per channel, for the channel display.
    pub touched: Signal<[Option<Param>; VOICES]>,
    /// Trigger counters per channel; the trigger LED keys its flash on these.
    pub hits: Signal<[u32; VOICES]>,
    pub drag: Signal<Option<Drag>>,
    /// One generator for RND and MUTATE, so rapid hits never share a seed.
    rng: Rc<RefCell<Noise>>,
}

impl Synth {
    pub fn new() -> Self {
        let initial = load().unwrap_or_default();
        Self {
            audio: AudioHandle::new(initial.to_cache()),
            state: Signal::new(initial),
            touched: Signal::new([None; VOICES]),
            hits: Signal::new([0; VOICES]),
            drag: Signal::new(None),
            rng: Rc::new(RefCell::new(Noise::new(random_seed()))),
        }
    }

    pub fn value(&self, target: Target) -> f32 {
        let s = self.state.read();
        match target {
            Target::Param(v, p) => s.voices[v][p.index()],
            Target::Master => s.master,
            Target::Mutate(v) => s.mutate[v],
        }
    }

    pub fn set(&self, target: Target, value: f32) {
        match target {
            Target::Param(voice, param) => self.set_param(voice, param, value),
            Target::Master => {
                let value = value.clamp(0.0, 1.0);
                let mut state = self.state;
                if state.peek().master != value {
                    state.write().master = value;
                    self.audio.send(Command::Master(value));
                }
            }
            Target::Mutate(voice) => {
                // UI-only: nothing to send, it acts when the channel is triggered.
                let value = value.clamp(0.0, 1.0);
                let mut state = self.state;
                if state.peek().mutate[voice] != value {
                    state.write().mutate[voice] = value;
                }
            }
        }
    }

    pub fn set_param(&self, voice: usize, param: Param, value: f32) {
        let value = param.sanitize(value);
        let mut state = self.state;
        if state.peek().voices[voice][param.index()] != value {
            state.write().voices[voice][param.index()] = value;
            self.audio.set_param(voice, param, value);
        }
        let mut touched = self.touched;
        if touched.peek()[voice] != Some(param) {
            touched.write()[voice] = Some(param);
        }
    }

    pub fn reset(&self, target: Target) {
        match target {
            Target::Param(_, p) => self.set(target, p.default_value()),
            Target::Master => self.set(target, frekussion_dsp::engine::DEFAULT_MASTER),
            Target::Mutate(_) => self.set(target, 0.0),
        }
        self.save();
    }

    pub fn load_patch(&self, voice: usize, patch: &VoicePatch) {
        self.apply_patch(voice, patch);
        self.save();
    }

    /// Load both channels from a kit.
    pub fn load_kit(&self, kit: &Kit) {
        for (voice, patch) in kit.patches().iter().enumerate() {
            self.apply_patch(voice, patch);
        }
        self.save();
    }

    /// Set a whole patch, sending only the values that changed. The channel
    /// display falls back to its summary (algorithm, tune, decay).
    fn apply_patch(&self, voice: usize, patch: &VoicePatch) {
        let mut state = self.state;
        for p in ALL_PARAMS {
            let value = p.sanitize(patch[p.index()]);
            if state.peek().voices[voice][p.index()] != value {
                state.write().voices[voice][p.index()] = value;
                self.audio.set_param(voice, p, value);
            }
        }
        let mut touched = self.touched;
        touched.write()[voice] = None;
    }

    /// Randomize the sound-shaping controls, leaving level, sense and pan alone.
    pub fn randomize(&self, voice: usize) {
        let current = self.state.peek().voices[voice];
        let patch = random_patch(&current, &mut self.rng.borrow_mut());
        self.load_patch(voice, &patch);
    }

    pub fn copy_voice(&self, from: usize, to: usize) {
        let mut patch = self.state.peek().voices[from];
        // Keep the destination's placement in the stereo field and its link.
        for p in [Param::Pan, Param::Link] {
            patch[p.index()] = self.state.peek().voices[to][p.index()];
        }
        self.load_patch(to, &patch);
    }

    /// The channels a hit on `voice` fires: itself plus any channel with
    /// LINK on. Mirrors `Engine::fired_by`.
    pub fn fired_by(&self, voice: usize) -> Vec<usize> {
        let state = self.state.peek();
        (0..VOICES)
            .filter(|&v| v == voice || state.voices[v][Param::Link.index()] >= 0.5)
            .collect()
    }

    /// Hit a channel (and the channels linked to it). With MUTATE above zero
    /// a firing channel's patch is mutated first, and the trigger snaps the
    /// voices so the new sound starts exactly on this hit rather than
    /// gliding in.
    pub fn trigger(&self, voice: usize, velocity: f32) {
        self.audio.user_gesture();
        let fired = self.fired_by(voice);
        let mut mutated = false;
        for &v in &fired {
            let amount = self.state.peek().mutate[v];
            if amount > 0.0 {
                let current = self.state.peek().voices[v];
                let patch = mutate_patch(&current, amount, &mut self.rng.borrow_mut());
                self.apply_patch(v, &patch);
                mutated = true;
            }
        }
        if mutated {
            self.save();
        }
        self.audio.trigger(voice, velocity, mutated);
        let mut hits = self.hits;
        for v in fired {
            hits.write()[v] += 1;
        }
    }

    pub fn save(&self) {
        save(&self.state.peek());
    }
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn random_seed() -> u32 {
    (js_sys::Math::random() * u32::MAX as f64) as u32
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn random_seed() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
        .unwrap_or(12345)
}

// --- persistence -----------------------------------------------------------

#[cfg(all(feature = "web", not(feature = "desktop")))]
const STORAGE_KEY: &str = "frekussion.state";

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn load() -> Option<PanelState> {
    let storage = web_sys::window()?.local_storage().ok()??;
    PanelState::deserialize(&storage.get_item(STORAGE_KEY).ok()??)
}

#[cfg(all(feature = "web", not(feature = "desktop")))]
fn save(state: &PanelState) {
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
        let _ = storage.set_item(STORAGE_KEY, &state.serialize());
    }
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn state_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("frekussion").join("state.txt"))
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn load() -> Option<PanelState> {
    PanelState::deserialize(&std::fs::read_to_string(state_path()?).ok()?)
}

#[cfg(not(all(feature = "web", not(feature = "desktop"))))]
fn save(state: &PanelState) {
    let Some(path) = state_path() else { return };
    let result = path
        .parent()
        .map(std::fs::create_dir_all)
        .unwrap_or(Ok(()))
        .and_then(|_| std::fs::write(&path, state.serialize()));
    if let Err(e) = result {
        eprintln!(
            "frekussion: could not save state to {}: {e}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sanitized(patch: &VoicePatch) -> bool {
        ALL_PARAMS
            .iter()
            .all(|p| p.sanitize(patch[p.index()]) == patch[p.index()])
    }

    #[test]
    fn mutate_off_changes_nothing() {
        let current = default_patch();
        assert_eq!(mutate_patch(&current, 0.0, &mut Noise::new(7)), current);
    }

    #[test]
    fn mutate_at_rnd_equals_the_rnd_button() {
        let current = default_patch();
        let a = mutate_patch(&current, 1.0, &mut Noise::new(7));
        let b = random_patch(&current, &mut Noise::new(7));
        assert_eq!(a, b);
        assert_ne!(a, current);
    }

    #[test]
    fn partial_mutation_moves_part_way_towards_the_random_patch() {
        let current = default_patch();
        for seed in 1..50 {
            let target = random_patch(&current, &mut Noise::new(seed));
            let half = mutate_patch(&current, 0.5, &mut Noise::new(seed));
            assert!(is_sanitized(&half));
            for p in ALL_PARAMS {
                let (c, t, h) = (current[p.index()], target[p.index()], half[p.index()]);
                match p.info().kind {
                    ParamKind::Continuous => {
                        assert!((h - (c + (t - c) * 0.5)).abs() < 1e-6, "{p:?}")
                    }
                    ParamKind::Choice(_) => assert_eq!(h, p.sanitize(c + (t - c) * 0.5), "{p:?}"),
                }
            }
        }
    }

    #[test]
    fn mutation_keeps_level_sense_pan_and_link() {
        let mut current = default_patch();
        current[Param::Level.index()] = 0.33;
        current[Param::Sense.index()] = 0.66;
        current[Param::Pan.index()] = 0.1;
        current[Param::Link.index()] = 1.0;
        let mut rng = Noise::new(3);
        for amount in [0.25, 0.5, 1.0] {
            let m = mutate_patch(&current, amount, &mut rng);
            for p in [Param::Level, Param::Sense, Param::Pan, Param::Link] {
                assert_eq!(m[p.index()], current[p.index()]);
            }
        }
    }

    #[test]
    fn random_patches_always_have_an_audible_carrier() {
        let mut rng = Noise::new(5);
        for _ in 0..500 {
            let p = random_patch(&default_patch(), &mut rng);
            let algo = ALGORITHMS[p[Param::Algorithm.index()] as usize];
            for op in 0..OPS {
                if algo.is_carrier(op) {
                    assert!(p[OpParam::Level.of(op).index()] >= 0.5);
                }
            }
        }
    }

    #[test]
    fn repeated_mutation_stays_in_range() {
        let mut patch = default_patch();
        let mut rng = Noise::new(11);
        for i in 0..1000 {
            patch = mutate_patch(&patch, [0.1, 0.5, 0.9][i % 3], &mut rng);
            assert!(is_sanitized(&patch));
        }
    }

    #[test]
    fn mutate_display_names_the_ends() {
        assert_eq!(mutate_display(0.0), "OFF");
        assert_eq!(mutate_display(0.5), "50%");
        assert_eq!(mutate_display(1.0), "RND");
    }

    #[test]
    fn state_roundtrip() {
        let mut s = PanelState {
            master: 0.33,
            mutate: [0.5, 1.0],
            ..Default::default()
        };
        s.voices[0][Param::Algorithm.index()] = 4.0;
        s.voices[1][Param::Tune.index()] = 0.123;
        let back = PanelState::deserialize(&s.serialize()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn rejects_foreign_text() {
        assert!(PanelState::deserialize("hello").is_none());
    }

    #[test]
    fn tolerates_garbage_lines() {
        let text = format!(
            "{FORMAT_HEADER}\nmaster 0.5\n0 Nope 1\n7 Tune 0.2\n0 Tune abc\n1 Algorithm 3\n"
        );
        let s = PanelState::deserialize(&text).unwrap();
        assert_eq!(s.master, 0.5);
        assert_eq!(s.voices[1][Param::Algorithm.index()], 3.0);
    }
}
