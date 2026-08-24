//! Emit pywal-compatible `colors.json` and the richer `palette.json`.

use crate::{
    color::{Oklab, Srgb},
    extract::{Assignment, Extraction, Mutation},
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use walbridge::palette::Palette;

/// Bright variant: bump lightness a bit while preserving hue/chroma.
fn brighten(color: Oklab) -> Oklab {
    color.with_lightness((color.l + 0.08).clamp(0.0, 1.0))
}

#[derive(Serialize)]
struct PywalColors {
    checksum: String,
    wallpaper: String,
    alpha: String,
    special: PywalSpecial,
    colors: PywalColorSlots,
}

#[derive(Serialize)]
struct PywalSpecial {
    background: String,
    foreground: String,
    cursor: String,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct PywalColorSlots {
    color0: String,
    color1: String,
    color2: String,
    color3: String,
    color4: String,
    color5: String,
    color6: String,
    color7: String,
    color8: String,
    color9: String,
    color10: String,
    color11: String,
    color12: String,
    color13: String,
    color14: String,
    color15: String,
}

pub fn write_colors_json(path: &Path, extraction: &Extraction) -> Result<()> {
    // Accents in pywal order: color1..color6 = red, green, yellow, blue, magenta, cyan.
    let a = &extraction.accents;
    let bg = extraction.background.srgb;
    let fg = extraction.foreground.srgb;

    let bright = |c: Srgb| brighten(c.to_oklab()).to_srgb();

    let pywal = PywalColors {
        checksum: short_checksum(&extraction.image_checksum),
        wallpaper: extraction.image_path.clone(),
        alpha: "100".into(),
        special: PywalSpecial {
            background: bg.hex_with_hash(),
            foreground: fg.hex_with_hash(),
            cursor: extraction.cursor.srgb.hex_with_hash(),
        },
        colors: PywalColorSlots {
            color0: bg.hex_with_hash(),
            color1: a[0].srgb.hex_with_hash(),
            color2: a[1].srgb.hex_with_hash(),
            color3: a[2].srgb.hex_with_hash(),
            color4: a[3].srgb.hex_with_hash(),
            color5: a[4].srgb.hex_with_hash(),
            color6: a[5].srgb.hex_with_hash(),
            color7: fg.hex_with_hash(),
            color8: bright(bg).hex_with_hash(),
            color9: bright(a[0].srgb).hex_with_hash(),
            color10: bright(a[1].srgb).hex_with_hash(),
            color11: bright(a[2].srgb).hex_with_hash(),
            color12: bright(a[3].srgb).hex_with_hash(),
            color13: bright(a[4].srgb).hex_with_hash(),
            color14: bright(a[5].srgb).hex_with_hash(),
            color15: bright(fg).hex_with_hash(),
        },
    };
    write_json(path, &pywal)
}

fn short_checksum(full: &str) -> String {
    // Pywal's `colors.json` uses a short md5-style string. Trim the
    // sha256 hex down so length matches ballpark, but keep the prefix so
    // ours is recognizable.
    full.trim_start_matches("sha256:")
        .chars()
        .take(32)
        .collect()
}

#[derive(Serialize)]
struct RichPalette<'a> {
    schema_version: u32,
    image: &'a str,
    image_checksum: &'a str,
    generated_at_unix: u64,
    extractor: Extractor,
    clusters: Vec<RichCluster>,
    assignments: RichAssignments,
    blacklist_applied: &'a [String],
}

#[derive(Serialize)]
struct Extractor {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct RichCluster {
    oklab: [f32; 3],
    srgb: String,
    weight: f32,
    rejected_by: Option<String>,
}

#[derive(Serialize)]
struct RichAssignments {
    background: RichAssignment,
    foreground: RichAssignment,
    cursor: RichAssignment,
    accent_red: RichAssignment,
    accent_green: RichAssignment,
    accent_yellow: RichAssignment,
    accent_blue: RichAssignment,
    accent_magenta: RichAssignment,
    accent_cyan: RichAssignment,
}

#[derive(Serialize)]
struct RichAssignment {
    srgb: String,
    oklab: [f32; 3],
    source_cluster: Option<usize>,
    mutation: String,
}

fn rich_from(a: &Assignment) -> RichAssignment {
    RichAssignment {
        srgb: a.srgb.hex_with_hash(),
        oklab: [a.oklab.l, a.oklab.a, a.oklab.b],
        source_cluster: a.source_cluster,
        mutation: match &a.mutation {
            Mutation::None => "none".into(),
            Mutation::LightnessClamp(l) => format!("lightness_clamp:{l:.3}"),
            Mutation::HueRotate { from, to } => format!("hue_rotate:{from:.1}->{to:.1}"),
            Mutation::ChromaBoost { from, to } => format!("chroma_boost:{from:.3}->{to:.3}"),
            Mutation::Synthesized => "synthesized".into(),
        },
    }
}

pub fn write_palette_json(path: &Path, extraction: &Extraction) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let clusters: Vec<RichCluster> = extraction
        .clusters
        .iter()
        .map(|c| RichCluster {
            oklab: [c.oklab.l, c.oklab.a, c.oklab.b],
            srgb: c.srgb.hex_with_hash(),
            weight: c.weight,
            rejected_by: c.rejected_by.clone(),
        })
        .collect();

    let rich = RichPalette {
        schema_version: 1,
        image: &extraction.image_path,
        image_checksum: &extraction.image_checksum,
        generated_at_unix: now,
        extractor: Extractor {
            name: "walbridge-extract",
            version: env!("CARGO_PKG_VERSION"),
        },
        clusters,
        assignments: RichAssignments {
            background: rich_from(&extraction.background),
            foreground: rich_from(&extraction.foreground),
            cursor: rich_from(&extraction.cursor),
            accent_red: rich_from(&extraction.accents[0]),
            accent_green: rich_from(&extraction.accents[1]),
            accent_yellow: rich_from(&extraction.accents[2]),
            accent_blue: rich_from(&extraction.accents[3]),
            accent_magenta: rich_from(&extraction.accents[4]),
            accent_cyan: rich_from(&extraction.accents[5]),
        },
        blacklist_applied: &extraction.blacklist_applied,
    };

    write_json(path, &rich)
}

/// Write the canonical Walbridge palette as a deterministic Tint-compatible
/// Base16 scheme for consumers such as Stylix.
pub fn write_base16_yaml(path: &Path, palette: &Palette) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }

    let mut contents =
        String::from("system: \"base16\"\nname: \"Walbridge\"\nauthor: \"walbridge-extract\"\n");
    contents.push_str(if palette.is_dark() {
        "variant: \"dark\"\n"
    } else {
        "variant: \"light\"\n"
    });
    contents.push_str("palette:\n");
    for (name, color) in palette.base16_colors() {
        contents.push_str(&format!("  {name}: \"{}\"\n", color.hashtag()));
    }

    std::fs::write(path, contents).with_context(|| format!("failed to write `{}`", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, bytes).with_context(|| format!("failed to write `{}`", path.display()))?;
    Ok(())
}
