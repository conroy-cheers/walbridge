//! Extractor configuration and blacklist.

use crate::color::Oklab;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_cluster_count")]
    pub cluster_count: usize,
    #[serde(default = "default_downsample_edge")]
    pub downsample_edge: u32,
    #[serde(default = "default_kmeans_iterations")]
    pub kmeans_iterations: usize,
    #[serde(default = "default_rng_seed")]
    pub rng_seed: u64,
    #[serde(default = "default_background_max_l")]
    pub background_max_l: f32,
    #[serde(default = "default_foreground_min_l")]
    pub foreground_min_l: f32,
    #[serde(default = "default_min_accent_chroma")]
    pub min_accent_chroma: f32,
    #[serde(default)]
    pub blacklist: Vec<BlacklistRegion>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlacklistRegion {
    pub name: String,
    /// OKLab L range, [min, max]. L is 0..~1.
    pub l: [f32; 2],
    /// OKLab a range.
    pub a: [f32; 2],
    /// OKLab b range.
    pub b: [f32; 2],
}

impl BlacklistRegion {
    pub fn contains(&self, c: Oklab) -> bool {
        (self.l[0]..=self.l[1]).contains(&c.l)
            && (self.a[0]..=self.a[1]).contains(&c.a)
            && (self.b[0]..=self.b[1]).contains(&c.b)
    }
}

fn default_cluster_count() -> usize {
    20
}
fn default_downsample_edge() -> u32 {
    400
}
fn default_kmeans_iterations() -> usize {
    30
}
fn default_rng_seed() -> u64 {
    0x5741_4c42_5249_4447 // "WALBRIDG"
}
fn default_background_max_l() -> f32 {
    0.22
}
fn default_foreground_min_l() -> f32 {
    0.80
}
fn default_min_accent_chroma() -> f32 {
    0.05
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cluster_count: default_cluster_count(),
            downsample_edge: default_downsample_edge(),
            kmeans_iterations: default_kmeans_iterations(),
            rng_seed: default_rng_seed(),
            background_max_l: default_background_max_l(),
            foreground_min_l: default_foreground_min_l(),
            min_accent_chroma: default_min_accent_chroma(),
            blacklist: builtin_blacklist(),
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let Some(p) = path else {
            return Ok(Self::default());
        };
        let text = fs::read_to_string(p)
            .with_context(|| format!("failed to read config `{}`", p.display()))?;
        let mut cfg: Self = toml::from_str(&text)
            .with_context(|| format!("failed to parse config `{}`", p.display()))?;
        // User blacklist supplements the builtins. If the user wants to
        // disable a builtin they can redefine it, or explicitly set
        // blacklist = [] (which suppresses builtins entirely).
        if cfg.blacklist.is_empty() && !text.contains("blacklist") {
            cfg.blacklist = builtin_blacklist();
        }
        Ok(cfg)
    }

    /// XDG lookup for config. Returns None if no file exists.
    pub fn default_config_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
            })?;
        let p = base.join("walbridge").join("extract.toml");
        p.exists().then_some(p)
    }
}

fn builtin_blacklist() -> Vec<BlacklistRegion> {
    vec![
        // The #19190a family: very dark, warm-yellow-tinted, low chroma.
        // This is the "olive mud" shade pywal extracted from green.jpg.
        BlacklistRegion {
            name: "olive mud".into(),
            l: [0.10, 0.38],
            a: [-0.04, 0.06],
            b: [0.015, 0.10],
        },
        // Mid-lightness desaturated yellow-green. Shows up on foliage
        // wallpapers and reads as sickly.
        BlacklistRegion {
            name: "baby-poop yellow-green".into(),
            l: [0.38, 0.60],
            a: [-0.10, 0.00],
            b: [0.06, 0.14],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Srgb;

    #[test]
    fn olive_mud_region_catches_original_bad_color() {
        let cfg = Config::default();
        let bad = Srgb::parse_hex("#19190a").unwrap().to_oklab();
        assert!(
            cfg.blacklist.iter().any(|r| r.contains(bad)),
            "default blacklist should reject #19190a"
        );
    }

    #[test]
    fn olive_mud_region_does_not_catch_pure_black() {
        let cfg = Config::default();
        let black = Srgb::parse_hex("#000000").unwrap().to_oklab();
        assert!(!cfg.blacklist.iter().any(|r| r.contains(black)));
    }

    #[test]
    fn olive_mud_region_does_not_catch_cool_dark_blue() {
        let cfg = Config::default();
        let dark_blue = Srgb::parse_hex("#0a1420").unwrap().to_oklab();
        assert!(!cfg.blacklist.iter().any(|r| r.contains(dark_blue)));
    }
}
