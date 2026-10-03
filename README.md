# Frekussion FK-2

A dual-channel **FM percussion synthesizer**, a sibling of the
[Synkussion SK-1](https://github.com/mkforsb/synkussion-sk1-slop). It keeps the
SK-1's layout: two independent channels, percussive envelopes, and a plate
reverb per channel. The analog model is replaced with frequency modulation:
each channel is a complete four-operator FM voice with its own noise
oscillator, and the two channels can modulate each other.

It's aimed at every kind of hit, from kicks, snares, toms and hats to metallic
clangs, bells, and hollow or discordant stabs.

It's written in Rust with a [Dioxus](https://dioxuslabs.com) UI, and it
builds for two targets from one codebase:

| Target | Audio | How |
| --- | --- | --- |
| Web (wasm) | WebAudio `AudioWorklet` | `make serve-web` |
| Native Linux | PulseAudio (also PipeWire's pulse server) | `make run-desktop` |

## Building

Requirements:

- Rust (stable) with `rustup target add wasm32-unknown-unknown`
- Dioxus CLI 0.7: `cargo binstall dioxus-cli` or grab `dx` from the Dioxus releases
- Desktop only (Debian/Ubuntu package names): `libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libpulse-dev pkg-config`

```sh
make serve-web      # http://127.0.0.1:8080, click once to enable audio
make run-desktop    # native window
make web desktop    # release bundles in target/dx/frekussion/release/
make test lint
make renders        # offline-render every preset and kit to ./renders/*.wav
make bench          # engine speed as a real-time factor
```

For the desktop build, `FREKUSSION_LATENCY_MS` (default 20) sets the
PulseAudio target latency.

## Playing it

- **Pads**: click a channel's TRIGGER pad. Where you click sets the velocity:
  near the top is a hard hit, near the bottom a soft one.
- **Keyboard**: `Z X C V` trigger channel 1 at increasing velocity.
  `N M , .` do the same for channel 2, and `Space` triggers both.
- **Knobs/faders**: drag vertically. Hold `Shift` for fine control, use the
  scroll wheel to nudge, and double-click to reset. A RATIO knob moves one
  ratio per scroll notch.
- **Algorithm**: click one of the numbered buttons, or click or scroll the
  diagram. Carriers are drawn in teal and modulators in pink. The operator
  rows below use the same colours, so you can see at a glance whether a
  LEVEL knob is a volume or a modulation index.
- **Presets** (grouped by kind), **RND** and **→ CHn** are in each channel's
  header. **Kits** in the top bar load both channels at once; most of them
  show off the cross-channel controls.
- **MUTATE** works as on the SK-1. Each hit moves the channel's patch that
  fraction of the way towards a fresh random patch; at **RND** every hit is a
  new sound. OUTPUT, SENSE, PAN and LINK are never touched.
- The whole panel is saved automatically: to `localStorage` on the web, and
  to `~/.config/frekussion/state.txt` on the desktop.

## The voice

```
            ┌──────── pitch: TUNE + SWEEP env + LFO ────────┐
            ▼                                               │
 other ch ─XMOD─►  OP4 ⟲ ─► OP3 ─► OP2 ─► OP1   (routing set  │
                   each with RATIO, LEVEL, DECAY  by ALGO)   │
 NOISE ─► SVF ─N>FM─►──────────────── carriers ─┐            │
   │                                            ▼            │
   │                          other ch ─RING─► × AMP env ─► to other ch
   │                                            │
   └─► × NOISE env ──────────────────────────►  + ─► DRIVE ─► FILTER ─► OUTPUT ─► PAN / PLATE
```

- **Operators**: four sine operators using phase modulation, as on the DX7.
  Every hit resets their phases, so each hit starts the same way (which
  matters for punchy kicks). Each operator has:
  - **RATIO**: steps through harmonic ratios (¼ to 16), plus the inharmonic
    partials of real percussion. These are the ideal circular membrane
    (1.59, 2.14, 2.30, 2.65, 2.92), the free-free bar (2.76, 5.40, 8.93,
    13.34), and the classic "clang" ratios √2, √3, π and 7.07.
  - **FINE**: ±200 cents, finer near the centre.
  - **LEVEL**: a carrier's volume, or a modulator's index. Full scale is about
    13 radians, which is roughly where a DX7 tops out.
  - **DECAY**: its own exponential envelope, 2 ms to 12 s. A short modulator
    decay gives the bright FM "click" of the attack; a long one keeps the
    tone clangorous. Carrier decays combine with the amp decay.
- **Algorithms** (8): from deep stacks (noisy, harsh), through branches and
  fan-ins (metallic), to two independent pairs, one modulator fanned out to
  three carriers (clusters and chords), and fully additive (bells, bars,
  808-style tone pairs). Operator 4 has a **feedback** loop. It averages its
  last two outputs, as the DX7 does, and above ~60 % it deliberately goes
  from sawtooth-like to noise.
- **Pitch**: TUNE (20 Hz–2.5 kHz), FINE, and a pitch **SWEEP** of ±6 octaves
  that applies to every operator. Right of centre drops onto the note (kicks,
  toms, zaps); left of centre rises into it (tabla-style "bwow").
- **Amp**: DECAY (10 ms–15 s). **VEL>FM** sets how much velocity scales the
  modulation index, so hard hits are brighter as well as louder. **DRIVE** is
  a soft saturator.
- **Noise**: its own oscillator with an LP/BP/HP state-variable filter
  (TONE, RES) and its own decay. It skips the amp envelope, so a short noise
  burst can sit on a long tone. **N>FM** lets the filtered noise
  phase-modulate the carriers, for snare rattle and grit.
- **Filter**: one bipolar knob. Left of centre is a lowpass (20 kHz → 40 Hz),
  right of centre a highpass (20 Hz → 8 kHz), and the centre is open.
- **LFO**: TRI/SQR/SAW/S&H to PITCH, FM (the modulation index) or AMP. It
  restarts on every hit, so it acts like part of the sound's envelope: a fast
  SAW on AMP gives buzz/flam textures, and S&H on PITCH gives glitchy bursts.
- **Cross**, between the two channels:
  - **XMOD**: the other channel's enveloped FM signal phase-modulates this
    channel's carriers, modulators or both. The channels usually sit at
    unrelated pitches, so this is a quick way to get inharmonic clangs. With
    XMOD up on both channels they modulate each other, and the result gets
    unstable in a useful way.
  - **RING**: ring modulation with the other channel.
  - **LINK**: this channel also fires when the other one is hit, so a pair
    can act as one layered instrument (see the *Body Snare* and *Cross Clang*
    kits). Linking is one level deep, and two channels linked to each other
    simply always fire together.

  XMOD and RING only do anything while the other channel is sounding. Each
  channel hears the other with a one-sample delay at the oversampled rate.
- **Reverb**: the SK-1's Dattorro plate, one per channel, fed post-fader
  (AMOUNT, DECAY, TONE, PRE).
- **Oversampling**: the voices run at 2×, and each is decimated by a 47-tap
  windowed-sinc filter. Envelope, filter and smoothing coefficients update
  every 16 samples; pitch is computed every sample so fast kick sweeps stay
  smooth. With everything running, the engine renders about 40× faster than
  real time on one desktop core (see `make bench`).

## Layout

```
crates/dsp       engine, voice, plate reverb, parameters, presets and kits; no dependencies, fully unit-tested
crates/worklet   C ABI over the engine (fk_*), compiled to a standalone wasm module with no imports
crates/app       Dioxus UI with `web` and `desktop` features
  build.rs         (web) builds crates/worklet for wasm32 and embeds it
  src/audio/web.rs     AudioContext + AudioWorkletNode; the module is compiled on the main thread
  src/audio/worklet.js the AudioWorkletProcessor that calls the wasm engine
  src/audio/pulse.rs   audio thread + pa_simple playback stream
```

As in the SK-1, the UI never touches the engine directly. It sends small
commands (set a parameter, trigger, master volume): over the worklet's
`MessagePort` on the web, and over an `mpsc` channel on the desktop.

## Inspiration and sources

- Elektron Model:Cycles, a 4-operator FM engine mapped into
  percussion-specific "machines" (kick, snare, metal, perc, tone, chord):
  [Sound On Sound review](https://www.soundonsound.com/reviews/elektron-modelcycles)
- MusicRadar, [metallic sounds with FM](https://www.musicradar.com/how-to/how-to-use-digital-synthesis-techniques-to-create-metallic-sounds)
  (nested modulators with cascading envelopes) and
  [drum sounds with alternative synthesis](https://www.musicradar.com/how-to/how-to-make-electronic-drum-sounds-using-alternative-synthesis-methods)
- ADSR, [drum synthesis with FM8](https://www.adsrsounds.com/fm8-tutorials/drum-synthesis-with-fm8-fundamentals-of-drum-synthesis/)
- AudioSpillage, [the FM percussion model](https://www.audiospillage.com/blog/?p=60)
- J. Dattorro, "Effect Design Part 1: Reverberator and Other Filters",
  JAES 45(9), 1997 (the plate)
