//! The panel: a header with the kit selector and master volume, and two
//! channel strips, each a complete four-operator FM voice.

use dioxus::prelude::*;
use frekussion_dsp::params::{ALGORITHMS, LFO_SAW, LFO_TRI, OpParam, ParamKind, map};
use frekussion_dsp::presets::{KITS, PRESETS};
use frekussion_dsp::{OPS, Param, VOICES};

use crate::audio::AudioStatus;
use crate::state::{Drag, Synth, Target};

const STYLE: &str = include_str!("../assets/style.css");

/// Pixels of vertical drag for a knob's full travel.
const KNOB_SPAN_PX: f64 = 200.0;
/// Fader geometry; keep in sync with `.fader-track` / `.fader-cap` in the CSS.
const FADER_TRACK_PX: f64 = 150.0;
const FADER_CAP_PX: f64 = 24.0;
const FADER_LEDS: usize = 10;
/// Toggle switch geometry; keep in sync with `.switch-slot` / `.switch-lever`.
const SWITCH_LEVER_PX: f64 = 14.0;
const SWITCH_LEVER_INSET_PX: f64 = 2.0;
/// Height of one position on a switch; keep in sync with `.switch-label`.
const SWITCH_STEP_PX: f64 = 14.0;
/// Trigger pad height; keep in sync with `.pad` in the CSS.
const PAD_PX: f64 = 96.0;

/// Keyboard map: (key, channel, velocity).
const KEYMAP: &[(Code, usize, f32)] = &[
    (Code::KeyZ, 0, 0.3),
    (Code::KeyX, 0, 0.55),
    (Code::KeyC, 0, 0.8),
    (Code::KeyV, 0, 1.0),
    (Code::KeyN, 1, 0.3),
    (Code::KeyM, 1, 0.55),
    (Code::Comma, 1, 0.8),
    (Code::Period, 1, 1.0),
];

#[component]
pub fn App() -> Element {
    let synth = use_context_provider(Synth::new);
    let drag = synth.drag;

    let s_move = synth.clone();
    let s_up = synth.clone();
    let s_down = synth.clone();
    let s_key = synth.clone();
    let end_drag = move || {
        let mut drag = s_up.drag;
        if drag.peek().is_some() {
            drag.set(None);
            s_up.save();
        }
    };
    let end_drag_leave = end_drag.clone();
    let end_drag_up = end_drag;

    rsx! {
        style { {STYLE} }
        div {
            class: if drag.read().is_some() { "root dragging" } else { "root" },
            tabindex: "0",
            autofocus: true,
            onpointerdown: move |_| s_down.audio.user_gesture(),
            onpointermove: move |e: PointerEvent| {
                let Some(d) = *s_move.drag.peek() else { return };
                e.prevent_default();
                let y = e.client_coordinates().y;
                let fine = e.modifiers().shift();
                let mut drag = s_move.drag;
                if fine != d.fine {
                    // Re-anchor so toggling SHIFT mid-drag doesn't jump.
                    drag.set(Some(Drag { start_y: y, start_value: s_move.value(d.target), fine, ..d }));
                    return;
                }
                let scale = if fine { 0.1 } else { 1.0 };
                let v = d.start_value as f64 + (d.start_y - y) / d.span_px * scale;
                s_move.set(d.target, v as f32);
            },
            onpointerup: move |_| end_drag_up(),
            onpointerleave: move |_| end_drag_leave(),
            onkeydown: move |e: KeyboardEvent| {
                let m = e.modifiers();
                if e.is_auto_repeating() || m.ctrl() || m.alt() || m.meta() {
                    return;
                }
                let code = e.code();
                if code == Code::Space {
                    e.prevent_default();
                    for v in 0..VOICES {
                        s_key.trigger(v, 1.0);
                    }
                } else if let Some(&(_, voice, vel)) = KEYMAP.iter().find(|(c, _, _)| *c == code) {
                    // Also stops a focused <select> from type-ahead jumping presets.
                    e.prevent_default();
                    s_key.trigger(voice, vel);
                } else {
                    s_key.audio.user_gesture();
                }
            },
            Header {}
            main { class: "channels",
                for v in 0..VOICES {
                    Channel { key: "{v}", voice: v }
                }
            }
            footer { class: "footer",
                span { "Keys: " }
                kbd { "Z X C V" }
                span { " channel 1 (soft → hard) · " }
                kbd { "N M , ." }
                span { " channel 2 · " }
                kbd { "Space" }
                span { " both · Drag knobs vertically (" }
                kbd { "Shift" }
                span { " = fine), scroll wheel to nudge, double-click to reset. Pads: hit higher for harder." }
                br {}
                span { "Operators: " }
                span { class: "legend car", "carrier" }
                span { " is heard, its LEVEL is volume · " }
                span { class: "legend mod", "modulator" }
                span { " shapes a carrier's timbre, its LEVEL is the modulation index." }
            }
        }
    }
}

#[component]
fn Header() -> Element {
    let synth = use_context::<Synth>();
    let status = synth.audio.status.read().clone();
    let (class, text) = match status {
        AudioStatus::NeedsGesture => (
            "status warn",
            "Click anywhere or press a key to start audio".to_string(),
        ),
        AudioStatus::Starting => ("status warn", "Starting audio…".to_string()),
        AudioStatus::Running {
            sample_rate,
            detail,
        } => (
            "status ok",
            format!("{detail} · {:.1} kHz", sample_rate as f32 / 1000.0),
        ),
        AudioStatus::Failed(e) => ("status err", format!("Audio unavailable: {e}")),
    };
    let s_kit = synth.clone();
    rsx! {
        header { class: "header",
            div { class: "brand",
                span { class: "logo", "FREKUSSION" }
                span { class: "model", "FK-2" }
                span { class: "tagline", "Dual FM Percussion Synthesizer" }
            }
            select {
                class: "preset kit",
                title: "Load a kit into both channels",
                onchange: move |e| {
                    if let Some(k) = e.value().parse::<usize>().ok().and_then(|i| KITS.get(i)) {
                        s_kit.load_kit(k);
                    }
                },
                option { value: "", disabled: true, selected: true, "Kit…" }
                for (i, k) in KITS.iter().enumerate() {
                    option { key: "{i}", value: "{i}", "{k.name}" }
                }
            }
            div { class: "{class}",
                span { class: "status-dot" }
                span { "{text}" }
            }
            div { class: "master",
                Knob { target: Target::Master, size: "sm" }
            }
        }
    }
}

/// Preset categories in menu order.
fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for p in PRESETS {
        if !out.contains(&p.category) {
            out.push(p.category);
        }
    }
    out
}

#[component]
fn Channel(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let other = (voice + 1) % VOICES;
    let s_preset = synth.clone();
    let s_rand = synth.clone();
    let s_copy = synth.clone();
    let p = |param: Param| Target::Param(voice, param);
    rsx! {
        section { class: "channel",
            div { class: "channel-head",
                h2 { "CH {voice + 1}" }
                select {
                    class: "preset",
                    title: "Load a factory preset",
                    onchange: move |e| {
                        if let Some(p) = e.value().parse::<usize>().ok().and_then(|i| PRESETS.get(i)) {
                            s_preset.load_patch(voice, &p.patch());
                        }
                    },
                    option { value: "", disabled: true, selected: true, "Preset…" }
                    for cat in categories() {
                        optgroup { key: "{cat}", label: "{cat}",
                            for (i, p) in PRESETS.iter().enumerate().filter(|(_, p)| p.category == cat) {
                                option { key: "{i}", value: "{i}", "{p.name}" }
                            }
                        }
                    }
                }
                button {
                    class: "btn",
                    title: "Randomize this channel",
                    onclick: move |_| s_rand.randomize(voice),
                    "RND"
                }
                button {
                    class: "btn",
                    title: "Copy this channel to channel {other + 1}",
                    onclick: move |_| s_copy.copy_voice(voice, other),
                    "→ CH{other + 1}"
                }
                Lcd { voice }
            }
            div { class: "channel-body",
                div { class: "sec sec-algo",
                    div { class: "sec-title", "ALGORITHM" }
                    AlgorithmSelector { voice }
                    div { class: "row",
                        Knob { target: p(Param::Feedback), size: "sm" }
                    }
                }
                div { class: "sec sec-ops",
                    div { class: "sec-title", "OPERATORS" }
                    Operators { voice }
                }
                div { class: "sec sec-pitch",
                    div { class: "sec-title", "PITCH" }
                    div { class: "row faders",
                        div { class: "tune-group",
                            Knob { target: p(Param::Fine), size: "sm" }
                            Fader { voice, param: Param::Tune }
                        }
                        div { class: "col",
                            Knob { target: p(Param::SweepAmount) }
                            Knob { target: p(Param::SweepTime), size: "sm" }
                        }
                    }
                }
                div { class: "sec sec-amp",
                    div { class: "sec-title", "AMP" }
                    div { class: "row faders",
                        Fader { voice, param: Param::Decay }
                        div { class: "col",
                            Knob { target: p(Param::VelFm) }
                            Knob { target: p(Param::Drive), size: "sm" }
                        }
                    }
                }
                div { class: "sec sec-out",
                    div { class: "row",
                        Knob { target: p(Param::Level) }
                        Knob { target: p(Param::Sense), size: "sm" }
                        Knob { target: p(Param::Pan), size: "sm" }
                    }
                    Pad { voice }
                    div { class: "row mutate-row",
                        Knob { target: Target::Mutate(voice), size: "sm" }
                    }
                }
                div { class: "sec sec-noise",
                    div { class: "sec-title", "NOISE" }
                    div { class: "row",
                        Knob { target: p(Param::NoiseLevel) }
                        Knob { target: p(Param::NoiseDecay) }
                        Knob { target: p(Param::NoiseTone) }
                        Knob { target: p(Param::NoiseRes), size: "sm" }
                        Switch { voice, param: Param::NoiseType }
                        Knob { target: p(Param::NoiseFm), size: "sm" }
                    }
                }
                div { class: "sec sec-filter",
                    div { class: "sec-title", "FILTER" }
                    div { class: "row",
                        Knob { target: p(Param::Filter) }
                        Knob { target: p(Param::Resonance), size: "sm" }
                    }
                }
                div { class: "sec sec-lfo",
                    div { class: "sec-title",
                        "LFO"
                        LfoLed { voice }
                    }
                    div { class: "row",
                        Knob { target: p(Param::LfoRate) }
                        Knob { target: p(Param::LfoDepth) }
                        Switch { voice, param: Param::LfoWave }
                        Switch { voice, param: Param::LfoDest }
                    }
                }
                div { class: "sec sec-cross",
                    div { class: "sec-title", "CROSS · FROM CH{other + 1}" }
                    div { class: "row",
                        Knob { target: p(Param::XmodAmount) }
                        Switch { voice, param: Param::XmodDest }
                        Knob { target: p(Param::Ring) }
                        Switch { voice, param: Param::Link }
                    }
                }
                div { class: "sec sec-reverb",
                    div { class: "sec-title", "REVERB" }
                    div { class: "row",
                        Knob { target: p(Param::ReverbMix) }
                        Knob { target: p(Param::ReverbDecay) }
                        Knob { target: p(Param::ReverbTone) }
                        Knob { target: p(Param::ReverbPredelay) }
                    }
                }
            }
        }
    }
}

/// Small read-out: the algorithm, and the last touched control.
#[component]
fn Lcd(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let state = synth.state.read();
    let patch = &state.voices[voice];
    let algo = patch[Param::Algorithm.index()] as usize;
    let line2 = match synth.touched.read()[voice] {
        Some(p) => format!(
            "{} {}",
            p.info().name.to_uppercase(),
            p.display(patch[p.index()])
        ),
        None => format!(
            "TUNE {} · DECAY {}",
            Param::Tune.display(patch[Param::Tune.index()]),
            Param::Decay.display(patch[Param::Decay.index()])
        ),
    };
    rsx! {
        div { class: "lcd",
            div { class: "lcd-line", "ALG {algo + 1}  {ALGORITHMS[algo].name}" }
            div { class: "lcd-line", "{line2}" }
        }
    }
}

/// The operator matrix: one row per operator, highest first, like the
/// signal flow in the stacked algorithms.
#[component]
fn Operators(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let algo_index = synth.value(Target::Param(voice, Param::Algorithm)) as usize;
    let algo = ALGORITHMS[algo_index.min(ALGORITHMS.len() - 1)];
    rsx! {
        div { class: "ops",
            div { class: "ops-head" }
            for w in OpParam::ALL {
                div { key: "{w:?}", class: "ops-head ctl-label", "{w.of(0).info().label}" }
            }
            for op in (0..OPS).rev() {
                OperatorRow { key: "{op}", voice, op, carrier: algo.is_carrier(op) }
            }
        }
    }
}

#[component]
fn OperatorRow(voice: usize, op: usize, carrier: bool) -> Element {
    let role = if carrier { "car" } else { "mod" };
    let title = if carrier {
        "Carrier: heard directly"
    } else {
        "Modulator: shapes the timbre of the operators it feeds"
    };
    rsx! {
        div { class: "op-id {role}", title: "{title}",
            span { class: "op-num", "{op + 1}" }
            span { class: "op-role", if carrier { "CAR" } else { "MOD" } }
        }
        for w in OpParam::ALL {
            Knob {
                key: "{w:?}",
                target: Target::Param(voice, w.of(op)),
                size: "xs",
                label: false,
                tone: role,
            }
        }
    }
}

/// Where each operator box sits in the diagram, per algorithm (viewBox
/// 100 × 100, operator 1 first).
const ALGO_LAYOUT: [[(f64, f64); OPS]; 8] = [
    [(50.0, 76.0), (50.0, 54.0), (50.0, 32.0), (50.0, 10.0)],
    [(50.0, 76.0), (50.0, 50.0), (32.0, 22.0), (68.0, 22.0)],
    [(50.0, 76.0), (30.0, 46.0), (70.0, 46.0), (70.0, 18.0)],
    [(50.0, 76.0), (20.0, 42.0), (50.0, 42.0), (80.0, 42.0)],
    [(30.0, 76.0), (30.0, 44.0), (70.0, 76.0), (70.0, 44.0)],
    [(20.0, 76.0), (50.0, 76.0), (80.0, 76.0), (50.0, 42.0)],
    [(18.0, 76.0), (18.0, 44.0), (50.0, 76.0), (82.0, 76.0)],
    [(14.0, 76.0), (38.0, 76.0), (62.0, 76.0), (86.0, 76.0)],
];
const OP_BOX_W: f64 = 18.0;
const OP_BOX_H: f64 = 13.0;
const OUT_Y: f64 = 95.0;

/// The ALGO selector: a diagram of the current routing (click or scroll to
/// step) and one button per algorithm.
#[component]
fn AlgorithmSelector(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let current = (synth.value(Target::Param(voice, Param::Algorithm)) as usize).min(7);
    let n = ALGORITHMS.len();
    let algo = ALGORITHMS[current];
    let layout = ALGO_LAYOUT[current];
    let mut edges = Vec::new();
    for to in 0..OPS {
        for from in 0..OPS {
            if algo.modulates(from, to) {
                let (x0, y0) = layout[from];
                let (x1, y1) = layout[to];
                edges.push((x0, y0 + OP_BOX_H / 2.0, x1, y1 - OP_BOX_H / 2.0));
            }
        }
    }
    let carriers: Vec<f64> = (0..OPS)
        .filter(|&op| algo.is_carrier(op))
        .map(|op| layout[op].0)
        .collect();
    let out_x0 = carriers.iter().cloned().fold(f64::MAX, f64::min);
    let out_x1 = carriers.iter().cloned().fold(f64::MIN, f64::max);
    let (fx, fy) = layout[OPS - 1];
    let fb_path = format!(
        "M {:.1} {:.1} h 5 v {:.1} h -5",
        fx + OP_BOX_W / 2.0,
        fy - OP_BOX_H / 2.0 + 3.0,
        -(OP_BOX_H / 2.0 + 3.0) - 1.0
    );
    let fb_on = synth.value(Target::Param(voice, Param::Feedback)) > 0.0;
    let s_body = synth.clone();
    let s_wheel = synth.clone();
    rsx! {
        div { class: "algo",
            svg {
                class: "algo-svg",
                view_box: "0 0 100 100",
                role: "img",
                "aria-label": "Algorithm {current + 1}: {algo.name}",
                onclick: move |_| {
                    s_body.set_param(voice, Param::Algorithm, ((current + 1) % n) as f32);
                    s_body.save();
                },
                onwheel: move |e: WheelEvent| {
                    e.prevent_default();
                    let dy = e.delta().strip_units().y;
                    if dy != 0.0 {
                        let next = (current as i32 + dy.signum() as i32).clamp(0, n as i32 - 1);
                        s_wheel.set_param(voice, Param::Algorithm, next as f32);
                        s_wheel.save();
                    }
                },
                title { "Algorithm {current + 1}: {algo.name} (click or scroll to change)" }
                for (i, (x0, y0, x1, y1)) in edges.into_iter().enumerate() {
                    line { key: "{i}", class: "algo-edge", x1: "{x0}", y1: "{y0}", x2: "{x1}", y2: "{y1}" }
                }
                path { class: if fb_on { "algo-fb on" } else { "algo-fb" }, d: "{fb_path}" }
                for (i, x) in carriers.iter().enumerate() {
                    line { key: "c{i}", class: "algo-out", x1: "{x}", y1: "{76.0 + OP_BOX_H / 2.0}", x2: "{x}", y2: "{OUT_Y}" }
                }
                line { class: "algo-out", x1: "{out_x0 - 4.0}", y1: "{OUT_Y}", x2: "{out_x1 + 4.0}", y2: "{OUT_Y}" }
                for op in 0..OPS {
                    g { key: "{op}",
                        rect {
                            class: if algo.is_carrier(op) { "algo-op car" } else { "algo-op mod" },
                            x: "{layout[op].0 - OP_BOX_W / 2.0}",
                            y: "{layout[op].1 - OP_BOX_H / 2.0}",
                            width: "{OP_BOX_W}",
                            height: "{OP_BOX_H}",
                            rx: "2",
                        }
                        text {
                            class: "algo-num",
                            x: "{layout[op].0}",
                            y: "{layout[op].1 + 3.5}",
                            text_anchor: "middle",
                            "{op + 1}"
                        }
                    }
                }
            }
            div { class: "algo-buttons",
                for i in 0..n {
                    AlgoButton { key: "{i}", voice, index: i, selected: i == current }
                }
            }
        }
    }
}

#[component]
fn AlgoButton(voice: usize, index: usize, selected: bool) -> Element {
    let synth = use_context::<Synth>();
    rsx! {
        button {
            class: if selected { "algo-btn sel" } else { "algo-btn" },
            title: "Algorithm {index + 1}: {ALGORITHMS[index].name}",
            onclick: move |_| {
                synth.set_param(voice, Param::Algorithm, index as f32);
                synth.save();
            },
            "{index + 1}"
        }
    }
}

fn polar(cx: f64, cy: f64, r: f64, deg: f64) -> (f64, f64) {
    let a = deg.to_radians();
    (cx + r * a.sin(), cy - r * a.cos())
}

fn arc_path(cx: f64, cy: f64, r: f64, from_deg: f64, to_deg: f64) -> String {
    let (x0, y0) = polar(cx, cy, r, from_deg);
    let (x1, y1) = polar(cx, cy, r, to_deg);
    let large = if (to_deg - from_deg).abs() > 180.0 {
        1
    } else {
        0
    };
    format!("M {x0:.2} {y0:.2} A {r} {r} 0 {large} 1 {x1:.2} {y1:.2}")
}

fn begin_drag(synth: &Synth, target: Target, e: &PointerEvent, span_px: f64) {
    e.prevent_default();
    synth.audio.user_gesture();
    if let Target::Param(v, p) = target {
        let mut touched = synth.touched;
        touched.write()[v] = Some(p);
    }
    let mut drag = synth.drag;
    drag.set(Some(Drag {
        target,
        start_y: e.client_coordinates().y,
        start_value: synth.value(target),
        span_px,
        fine: e.modifiers().shift(),
    }));
}

fn nudge(synth: &Synth, target: Target, e: &WheelEvent) {
    e.prevent_default();
    let dy = e.delta().strip_units().y;
    if dy == 0.0 {
        return;
    }
    // Stepped RATIO knobs move one ratio per notch.
    let step = match target {
        Target::Param(_, p)
            if matches!(
                frekussion_dsp::params::op_param(p),
                Some((_, OpParam::Ratio))
            ) =>
        {
            1.0 / (frekussion_dsp::params::RATIOS.len() - 1) as f32
        }
        _ if e.modifiers().shift() => 0.002,
        _ => 0.02,
    };
    let v = synth.value(target) - (dy.signum() as f32) * step;
    synth.set(target, v);
    synth.save();
}

fn target_info(target: Target) -> (String, String, f32) {
    match target {
        Target::Param(_, p) => (
            p.info().label.to_string(),
            p.info().name.to_string(),
            p.default_value(),
        ),
        Target::Master => (
            "MASTER".into(),
            "Master volume".into(),
            frekussion_dsp::engine::DEFAULT_MASTER,
        ),
        Target::Mutate(_) => ("MUTATE".into(), "Mutate on trigger".into(), 0.0),
    }
}

fn display(target: Target, value: f32) -> String {
    match target {
        Target::Param(_, p) => p.display(value),
        Target::Master => Param::Level.display(value),
        Target::Mutate(_) => crate::state::mutate_display(value),
    }
}

/// A rotary control. `size` is `lg`, `sm` or `xs`; `tone` picks the arc
/// colour (`car`/`mod` for operators).
#[component]
fn Knob(
    target: Target,
    #[props(default = "lg")] size: &'static str,
    #[props(default = true)] label: bool,
    #[props(default = "")] tone: &'static str,
) -> Element {
    let synth = use_context::<Synth>();
    let value = synth.value(target);
    let (label_text, name, _) = target_info(target);
    let active = matches!(*synth.drag.read(), Some(d) if d.target == target);
    let angle = -135.0 + 270.0 * value as f64;
    let (px, py) = polar(30.0, 30.0, 15.0, angle);
    let track = arc_path(30.0, 30.0, 26.0, -135.0, 135.0);
    // Bipolar controls light the arc from the centre.
    let bipolar = matches!(target, Target::Param(_, p) if p.is_bipolar());
    let (a0, a1) = if bipolar {
        (angle.min(0.0), angle.max(0.0))
    } else {
        (-135.0, angle)
    };
    let lit = if (a1 - a0).abs() > 0.5 {
        arc_path(30.0, 30.0, 26.0, a0, a1)
    } else {
        String::new()
    };
    let text = display(target, value);
    let s_down = synth.clone();
    let s_dbl = synth.clone();
    let s_wheel = synth.clone();
    rsx! {
        div { class: "knob knob-{size} {tone}", class: if active { "active" },
            if label {
                div { class: "ctl-label", "{label_text}" }
            }
            svg {
                class: "knob-svg",
                view_box: "0 0 60 60",
                role: "slider",
                "aria-label": "{name}",
                "aria-valuetext": "{text}",
                onpointerdown: move |e| begin_drag(&s_down, target, &e, KNOB_SPAN_PX),
                ondoubleclick: move |_| s_dbl.reset(target),
                onwheel: move |e| nudge(&s_wheel, target, &e),
                title { "{name}: {text}" }
                path { class: "knob-track", d: "{track}" }
                if !lit.is_empty() {
                    path { class: "knob-lit", d: "{lit}" }
                }
                circle { class: "knob-skirt", cx: "30", cy: "30", r: "21" }
                circle { class: "knob-body", cx: "30", cy: "30", r: "17" }
                line { class: "knob-pointer", x1: "30", y1: "30", x2: "{px:.2}", y2: "{py:.2}" }
            }
            div { class: "ctl-value", "{text}" }
        }
    }
}

#[component]
fn Fader(voice: usize, param: Param) -> Element {
    let synth = use_context::<Synth>();
    let target = Target::Param(voice, param);
    let value = synth.value(target);
    let info = param.info();
    let text = param.display(value);
    let active = matches!(*synth.drag.read(), Some(d) if d.target == target);
    let travel = FADER_TRACK_PX - FADER_CAP_PX;
    let cap_top = (1.0 - value as f64) * travel;
    let s_down = synth.clone();
    let s_dbl = synth.clone();
    let s_wheel = synth.clone();
    rsx! {
        div { class: "fader", class: if active { "active" },
            div { class: "ctl-label", "{info.label}" }
            div {
                class: "fader-body",
                role: "slider",
                title: "{info.name}: {text}",
                "aria-label": "{info.name}",
                "aria-valuetext": "{text}",
                onpointerdown: move |e| begin_drag(&s_down, target, &e, travel),
                ondoubleclick: move |_| s_dbl.reset(target),
                onwheel: move |e| nudge(&s_wheel, target, &e),
                div { class: "fader-leds",
                    for i in (0..FADER_LEDS).rev() {
                        div {
                            key: "{i}",
                            class: if value * FADER_LEDS as f32 >= i as f32 + 0.5 { "led on" } else { "led" },
                        }
                    }
                }
                div { class: "fader-track",
                    div { class: "fader-slot" }
                    div { class: "fader-cap", style: "top: {cap_top:.1}px" }
                }
            }
            div { class: "ctl-value", "{text}" }
        }
    }
}

/// Slide switch for choice parameters, one position per option; click a
/// label to select it or the slot to step.
#[component]
fn Switch(voice: usize, param: Param) -> Element {
    let synth = use_context::<Synth>();
    let ParamKind::Choice(names) = param.info().kind else {
        return rsx! {};
    };
    let current = synth.value(Target::Param(voice, param)) as usize;
    let n = names.len();
    // Two-way switches read ON at the top like the hardware; longer ones
    // list their options top to bottom.
    let order: Vec<usize> = if n == 2 { vec![1, 0] } else { (0..n).collect() };
    let row = order.iter().position(|&i| i == current).unwrap_or(0);
    let slot_px = n as f64 * SWITCH_STEP_PX + 2.0 * SWITCH_LEVER_INSET_PX;
    let lever_px = SWITCH_LEVER_INSET_PX + row as f64 * SWITCH_STEP_PX;
    let s_cycle = synth.clone();
    rsx! {
        div { class: "switch",
            div { class: "ctl-label", "{param.info().label}" }
            div { class: "switch-body",
                div {
                    class: "switch-slot",
                    style: "height: {slot_px:.0}px",
                    title: "{param.info().name}",
                    onclick: move |_| {
                        s_cycle.set_param(voice, param, ((current + 1) % n) as f32);
                        s_cycle.save();
                    },
                    div { class: "switch-lever", style: "top: {lever_px:.1}px; height: {SWITCH_LEVER_PX}px" }
                }
                div { class: "switch-labels",
                    for i in order {
                        SwitchLabel { key: "{i}", voice, param, index: i, name: names[i], selected: i == current }
                    }
                }
            }
        }
    }
}

#[component]
fn SwitchLabel(
    voice: usize,
    param: Param,
    index: usize,
    name: &'static str,
    selected: bool,
) -> Element {
    let synth = use_context::<Synth>();
    rsx! {
        div {
            class: if selected { "switch-label sel" } else { "switch-label" },
            onclick: move |_| {
                synth.set_param(voice, param, index as f32);
                synth.save();
            },
            "{name}"
        }
    }
}

/// Blinks or pulses at the LFO rate while the LFO has depth.
#[component]
fn LfoLed(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let state = synth.state.read();
    let patch = &state.voices[voice];
    let wave = patch[Param::LfoWave.index()] as u32;
    let depth = patch[Param::LfoDepth.index()];
    let period = 1.0 / map::lfo_hz(patch[Param::LfoRate.index()]);
    let (class, style) = if depth <= 0.0 {
        ("led lfo-led", String::new())
    } else if wave == LFO_TRI || wave == LFO_SAW {
        (
            "led lfo-led pulse",
            format!("animation-duration: {period:.3}s"),
        )
    } else {
        (
            "led lfo-led blink",
            format!("animation-duration: {period:.3}s"),
        )
    };
    rsx! {
        span { class: "{class}", style: "{style}", title: "LFO" }
    }
}

#[component]
fn Pad(voice: usize) -> Element {
    let synth = use_context::<Synth>();
    let hits = synth.hits.read()[voice];
    let linked = synth.state.read().voices[voice][Param::Link.index()] >= 0.5;
    rsx! {
        div {
            class: "pad",
            role: "button",
            title: "Trigger (hit higher for a harder hit)",
            onpointerdown: move |e: PointerEvent| {
                e.prevent_default();
                let y = e.element_coordinates().y;
                let velocity = (1.0 - y / PAD_PX).clamp(0.05, 1.0) as f32;
                synth.trigger(voice, velocity);
            },
            for h in std::iter::once(hits) {
                span { key: "{h}", class: if h > 0 { "led trig-led flash" } else { "led trig-led" } }
            }
            span { class: "pad-label", "TRIGGER" }
            span { class: "pad-hint", "hard" }
            span { class: "pad-hint soft", "soft" }
            if linked {
                span { class: "pad-link", title: "Also fires when the other channel is hit", "LINKED" }
            }
        }
    }
}
