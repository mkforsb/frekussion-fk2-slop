//! Real-time factor of the engine: both voices with all four operators,
//! noise and cross modulation running, with and without the reverbs.
//!
//! ```sh
//! cargo run -p frekussion-dsp --release --example bench
//! ```

use frekussion_dsp::params::OpParam;
use frekussion_dsp::{Engine, Param};
use std::time::Instant;

fn main() {
    let sr = 48_000.0;
    let seconds = 60.0;
    for reverb in [0.0, 0.6] {
        let mut e = Engine::new(sr);
        for v in 0..2 {
            e.set_param(v, Param::Algorithm, 0.0);
            for op in 0..4 {
                e.set_param(v, OpParam::Level.of(op), 0.6);
            }
            e.set_param(v, Param::Feedback, 0.5);
            e.set_param(v, Param::NoiseLevel, 0.5);
            e.set_param(v, Param::XmodAmount, 0.5);
            e.set_param(v, Param::Decay, 0.9);
            e.set_param(v, Param::ReverbMix, reverb);
        }
        let mut l = [0.0f32; 128];
        let mut r = [0.0f32; 128];
        let blocks = (sr * seconds / 128.0) as usize;
        let start = Instant::now();
        for b in 0..blocks {
            if b % 150 == 0 {
                e.trigger(0, 1.0);
                e.trigger(1, 0.7);
            }
            e.render(&mut l, &mut r);
        }
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "reverb amount {reverb}: {seconds} s of audio in {elapsed:.3} s -> {:.0}x real time",
            seconds as f64 / elapsed
        );
    }
}
