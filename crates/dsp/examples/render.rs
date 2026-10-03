//! Render every factory preset (soft hit, then hard hit) and every kit
//! (channel 1, channel 2, both, at two velocities) to WAV files.
//!
//! ```sh
//! cargo run -p frekussion-dsp --release --example render -- out_dir
//! ```

use std::io::Write;

use frekussion_dsp::Engine;
use frekussion_dsp::presets::{KITS, PRESETS};

const SR: u32 = 48_000;

fn write_wav(path: &std::path::Path, left: &[f32], right: &[f32]) -> std::io::Result<()> {
    let frames = left.len() as u32;
    let data_len = frames * 4;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&SR.to_le_bytes())?;
    f.write_all(&(SR * 4).to_le_bytes())?;
    f.write_all(&4u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for (l, r) in left.iter().zip(right) {
        for s in [l, r] {
            f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
        }
    }
    f.flush()
}

fn file_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

fn main() -> std::io::Result<()> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "renders".into()));
    std::fs::create_dir_all(dir.join("kits"))?;
    for preset in PRESETS {
        let mut engine = Engine::new(SR as f32);
        engine.load_patch(0, &preset.patch());
        engine.snap_params();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for velocity in [0.35, 1.0] {
            engine.trigger(0, velocity);
            let (l, r) = engine.render_vec(SR as usize * 3 / 2);
            left.extend(l);
            right.extend(r);
        }
        let path = dir.join(format!("{}.wav", file_name(preset.name)));
        write_wav(&path, &left, &right)?;
        println!(
            "{:<18} peak {:>6.3}  -> {}",
            preset.name,
            peak(&left).max(peak(&right)),
            path.display()
        );
    }
    for kit in KITS {
        let mut engine = Engine::new(SR as f32);
        for (v, patch) in kit.patches().iter().enumerate() {
            engine.load_patch(v, patch);
        }
        engine.snap_params();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for velocity in [0.5, 1.0] {
            for hit in [&[0][..], &[1], &[0, 1]] {
                for &v in hit {
                    engine.trigger(v, velocity);
                }
                let (l, r) = engine.render_vec(SR as usize);
                left.extend(l);
                right.extend(r);
            }
        }
        let path = dir
            .join("kits")
            .join(format!("{}.wav", file_name(kit.name)));
        write_wav(&path, &left, &right)?;
        println!(
            "kit {:<14} peak {:>6.3}  -> {}",
            kit.name,
            peak(&left).max(peak(&right)),
            path.display()
        );
    }
    Ok(())
}
