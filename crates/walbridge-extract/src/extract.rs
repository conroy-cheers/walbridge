//! Top-level extraction: image → clusters → slot assignments.

use crate::{
    cluster::{kmeans, Cluster},
    color::{Oklab, Srgb},
    config::{BlacklistRegion, Config},
};
use anyhow::{Context, Result};
use image::{imageops::FilterType, GenericImageView};
use sha2::{Digest, Sha256};
use std::path::Path;

/// A clustered color centroid with its source weight and rejection state.
#[derive(Debug, Clone)]
pub struct RankedCluster {
    pub oklab: Oklab,
    pub srgb: Srgb,
    /// Fraction of (downsampled) pixels in this cluster, 0..1.
    pub weight: f32,
    /// If Some, the name of the blacklist region that ejected this cluster.
    pub rejected_by: Option<String>,
}

/// How a final slot was derived from its chosen cluster.
#[derive(Debug, Clone)]
pub enum Mutation {
    /// Cluster was used as-is.
    None,
    /// Lightness was clamped to this value.
    LightnessClamp(f32),
    /// Hue rotated to target_deg to satisfy an accent role.
    HueRotate { from: f32, to: f32 },
    /// Chroma boosted to a minimum; `from` and `to` are raw chroma values.
    ChromaBoost { from: f32, to: f32 },
    /// No suitable cluster existed; slot was synthesized from foreground/bg.
    Synthesized,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub srgb: Srgb,
    pub oklab: Oklab,
    /// Index into `RankedCluster` list that sourced this slot. `None` if synthesized.
    pub source_cluster: Option<usize>,
    pub mutation: Mutation,
}

#[derive(Debug, Clone)]
pub struct Extraction {
    pub image_path: String,
    pub image_checksum: String,
    pub clusters: Vec<RankedCluster>,
    pub background: Assignment,
    pub foreground: Assignment,
    pub cursor: Assignment,
    /// 6 accents: red, green, yellow, blue, magenta, cyan (pywal color1..color6).
    pub accents: [Assignment; 6],
    pub blacklist_applied: Vec<String>,
}

/// Target hue (OKLab degrees) for each of the six pywal accent slots.
/// Derived empirically from pure sRGB primaries/secondaries in OKLab.
const ACCENT_TARGETS: [(&str, f32); 6] = [
    ("red", 29.0),
    ("green", 142.0),
    ("yellow", 109.0),
    ("blue", 264.0),
    ("magenta", 328.0),
    ("cyan", 196.0),
];

/// Angular distance between two hue angles in degrees, 0..180.
fn hue_delta(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

pub fn extract(image_path: &Path, config: &Config) -> Result<Extraction> {
    let bytes = std::fs::read(image_path)
        .with_context(|| format!("failed to read image `{}`", image_path.display()))?;
    let checksum = {
        let mut h = Sha256::new();
        h.update(&bytes);
        let digest = h.finalize();
        let mut hex = String::with_capacity(7 + digest.len() * 2);
        hex.push_str("sha256:");
        for byte in digest.iter() {
            use std::fmt::Write as _;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        hex
    };

    let img = image::load_from_memory(&bytes)
        .with_context(|| format!("failed to decode image `{}`", image_path.display()))?;
    let (w, h) = img.dimensions();
    let long = w.max(h);
    let scaled = if long > config.downsample_edge {
        let scale = config.downsample_edge as f32 / long as f32;
        img.resize(
            (w as f32 * scale) as u32,
            (h as f32 * scale) as u32,
            FilterType::Lanczos3,
        )
    } else {
        img
    };
    let rgb = scaled.to_rgb8();

    let samples: Vec<Oklab> = rgb
        .pixels()
        .map(|p| Srgb { r: p[0], g: p[1], b: p[2] }.to_oklab())
        .collect();

    let clusters = kmeans(
        &samples,
        config.cluster_count,
        config.kmeans_iterations,
        config.rng_seed,
    );

    let total: usize = clusters.iter().map(|c| c.count).sum();
    let total_f = total.max(1) as f32;

    let mut ranked: Vec<RankedCluster> = clusters
        .into_iter()
        .map(|c: Cluster| {
            let rejected = config
                .blacklist
                .iter()
                .find(|r| r.contains(c.centroid))
                .map(|r: &BlacklistRegion| r.name.clone());
            RankedCluster {
                oklab: c.centroid,
                srgb: c.centroid.to_srgb(),
                weight: c.count as f32 / total_f,
                rejected_by: rejected,
            }
        })
        .collect();

    // Sort by weight descending so the richer output is dominance-ranked.
    ranked.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());

    let background = pick_background(&ranked, config)?;
    let foreground = pick_foreground(&ranked, config, &background)?;
    let cursor = foreground.clone();

    let accents = [
        pick_accent(&ranked, config, 0, &background)?,
        pick_accent(&ranked, config, 1, &background)?,
        pick_accent(&ranked, config, 2, &background)?,
        pick_accent(&ranked, config, 3, &background)?,
        pick_accent(&ranked, config, 4, &background)?,
        pick_accent(&ranked, config, 5, &background)?,
    ];

    let mut applied: Vec<String> = config
        .blacklist
        .iter()
        .filter(|r| ranked.iter().any(|c| c.rejected_by.as_deref() == Some(r.name.as_str())))
        .map(|r| r.name.clone())
        .collect();
    applied.sort();
    applied.dedup();

    Ok(Extraction {
        image_path: image_path.display().to_string(),
        image_checksum: checksum,
        clusters: ranked,
        background,
        foreground,
        cursor,
        accents,
        blacklist_applied: applied,
    })
}

fn pick_background(ranked: &[RankedCluster], cfg: &Config) -> Result<Assignment> {
    // Prefer the highest-weight surviving cluster whose L is already dark enough.
    if let Some((idx, c)) = ranked
        .iter()
        .enumerate()
        .filter(|(_, c)| c.rejected_by.is_none() && c.oklab.l <= cfg.background_max_l)
        .next()
    {
        return Ok(Assignment {
            srgb: c.srgb,
            oklab: c.oklab,
            source_cluster: Some(idx),
            mutation: Mutation::None,
        });
    }
    // Else take the darkest surviving cluster and clamp its L down.
    let (idx, c) = ranked
        .iter()
        .enumerate()
        .filter(|(_, c)| c.rejected_by.is_none())
        .min_by(|(_, a), (_, b)| a.oklab.l.partial_cmp(&b.oklab.l).unwrap())
        .context("no surviving cluster for background")?;
    let darkened = c.oklab.with_lightness(cfg.background_max_l);
    Ok(Assignment {
        srgb: darkened.to_srgb(),
        oklab: darkened,
        source_cluster: Some(idx),
        mutation: Mutation::LightnessClamp(cfg.background_max_l),
    })
}

fn pick_foreground(
    ranked: &[RankedCluster],
    cfg: &Config,
    background: &Assignment,
) -> Result<Assignment> {
    // Lightest surviving cluster above the foreground threshold.
    if let Some((idx, c)) = ranked
        .iter()
        .enumerate()
        .filter(|(_, c)| c.rejected_by.is_none() && c.oklab.l >= cfg.foreground_min_l)
        .max_by(|(_, a), (_, b)| a.oklab.l.partial_cmp(&b.oklab.l).unwrap())
    {
        return Ok(Assignment {
            srgb: c.srgb,
            oklab: c.oklab,
            source_cluster: Some(idx),
            mutation: Mutation::None,
        });
    }
    // Else take the lightest surviving cluster and push L up.
    if let Some((idx, c)) = ranked
        .iter()
        .enumerate()
        .filter(|(_, c)| c.rejected_by.is_none())
        .max_by(|(_, a), (_, b)| a.oklab.l.partial_cmp(&b.oklab.l).unwrap())
    {
        let lifted = c.oklab.with_lightness(cfg.foreground_min_l);
        return Ok(Assignment {
            srgb: lifted.to_srgb(),
            oklab: lifted,
            source_cluster: Some(idx),
            mutation: Mutation::LightnessClamp(cfg.foreground_min_l),
        });
    }
    // No clusters survived blacklist. Synthesize from background.
    let synth = background.oklab.with_lightness(cfg.foreground_min_l);
    Ok(Assignment {
        srgb: synth.to_srgb(),
        oklab: synth,
        source_cluster: None,
        mutation: Mutation::Synthesized,
    })
}

fn pick_accent(
    ranked: &[RankedCluster],
    cfg: &Config,
    accent_idx: usize,
    background: &Assignment,
) -> Result<Assignment> {
    let (_name, target_hue) = ACCENT_TARGETS[accent_idx];

    // Score surviving clusters: prefer near-target hue + adequate chroma.
    // Weighted: hue error is primary, chroma secondary, weight as tie-breaker.
    let best = ranked
        .iter()
        .enumerate()
        .filter(|(_, c)| c.rejected_by.is_none())
        .filter(|(_, c)| c.oklab.chroma() >= cfg.min_accent_chroma * 0.5)
        .min_by(|(_, a), (_, b)| {
            let score = |c: &RankedCluster| {
                let hue_err = hue_delta(c.oklab.hue_deg(), target_hue);
                // Penalty for being below the min chroma.
                let chroma_pen =
                    (cfg.min_accent_chroma - c.oklab.chroma()).max(0.0) * 200.0;
                // Reward weight mildly.
                hue_err + chroma_pen - c.weight * 20.0
            };
            score(a).partial_cmp(&score(b)).unwrap()
        });

    if let Some((idx, c)) = best {
        let hue_err = hue_delta(c.oklab.hue_deg(), target_hue);
        let mut adjusted = c.oklab;
        let mut mutation = Mutation::None;
        if hue_err > 35.0 {
            let from = c.oklab.hue_deg();
            adjusted = adjusted.with_hue(target_hue);
            mutation = Mutation::HueRotate { from, to: target_hue };
        }
        if adjusted.chroma() < cfg.min_accent_chroma {
            let from = adjusted.chroma();
            adjusted = adjusted.with_chroma(cfg.min_accent_chroma);
            // Preserve earlier hue-rotate if one happened; record whichever
            // mutation is the most informative.
            if matches!(mutation, Mutation::None) {
                mutation = Mutation::ChromaBoost { from, to: cfg.min_accent_chroma };
            }
        }
        return Ok(Assignment {
            srgb: adjusted.to_srgb(),
            oklab: adjusted,
            source_cluster: Some(idx),
            mutation,
        });
    }

    // Synthesize from the background tone: set hue to target, lift L to a
    // reasonable accent lightness, and set chroma.
    let synth = Oklab::new(0.70, 0.0, 0.0)
        .with_hue(target_hue)
        .with_chroma(cfg.min_accent_chroma.max(0.10));
    let _ = background;
    Ok(Assignment {
        srgb: synth.to_srgb(),
        oklab: synth,
        source_cluster: None,
        mutation: Mutation::Synthesized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_delta_wraps() {
        assert!((hue_delta(10.0, 350.0) - 20.0).abs() < 0.001);
        assert!((hue_delta(350.0, 10.0) - 20.0).abs() < 0.001);
        assert!((hue_delta(180.0, 0.0) - 180.0).abs() < 0.001);
    }
}
