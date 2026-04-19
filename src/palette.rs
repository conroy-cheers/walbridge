use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Deserialize)]
struct WalData {
    checksum: Option<String>,
    wallpaper: String,
    special: WalSpecial,
    colors: WalColors,
}

#[derive(Debug, Deserialize)]
struct WalSpecial {
    background: String,
    foreground: String,
    cursor: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WalColors {
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

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim().trim_start_matches('#');
        anyhow::ensure!(trimmed.len() == 6, "invalid color `{input}`");

        Ok(Self {
            r: u8::from_str_radix(&trimmed[0..2], 16).context("invalid red channel")?,
            g: u8::from_str_radix(&trimmed[2..4], 16).context("invalid green channel")?,
            b: u8::from_str_radix(&trimmed[4..6], 16).context("invalid blue channel")?,
        })
    }

    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn hashtag(self) -> String {
        format!("#{}", self.hex())
    }

    pub fn rgb_csv(self) -> String {
        format!("{}, {}, {}", self.r, self.g, self.b)
    }

    pub fn with_alpha(self, alpha: u8) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", alpha, self.r, self.g, self.b)
    }

    pub fn mix(self, other: Color, ratio: f32) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        let inv = 1.0 - ratio;

        Self {
            r: ((self.r as f32 * inv) + (other.r as f32 * ratio)).round() as u8,
            g: ((self.g as f32 * inv) + (other.g as f32 * ratio)).round() as u8,
            b: ((self.b as f32 * inv) + (other.b as f32 * ratio)).round() as u8,
        }
    }

    pub fn lighten(self, ratio: f32) -> Self {
        self.mix(
            Color {
                r: 255,
                g: 255,
                b: 255,
            },
            ratio,
        )
    }

    pub fn to_hsl(self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < f32::EPSILON {
            return (0.0, 0.0, l);
        }

        let d = max - min;
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        let h = if (max - r).abs() < f32::EPSILON {
            60.0 * (((g - b) / d).rem_euclid(6.0))
        } else if (max - g).abs() < f32::EPSILON {
            60.0 * (((b - r) / d) + 2.0)
        } else {
            60.0 * (((r - g) / d) + 4.0)
        };

        (h, s, l)
    }

    pub fn from_hsl(h: f32, s: f32, l: f32) -> Self {
        let h = h.rem_euclid(360.0);
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - (((h / 60.0).rem_euclid(2.0)) - 1.0).abs());
        let m = l - c / 2.0;

        let (r1, g1, b1) = match h {
            h if h < 60.0 => (c, x, 0.0),
            h if h < 120.0 => (x, c, 0.0),
            h if h < 180.0 => (0.0, c, x),
            h if h < 240.0 => (0.0, x, c),
            h if h < 300.0 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        Self {
            r: ((r1 + m) * 255.0).round() as u8,
            g: ((g1 + m) * 255.0).round() as u8,
            b: ((b1 + m) * 255.0).round() as u8,
        }
    }

    pub fn luminance(self) -> f32 {
        let channel = |v: u8| {
            let c = v as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };

        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

#[derive(Debug)]
pub struct Palette {
    pub checksum: String,
    pub wallpaper: String,
    pub cursor: Color,
    pub background: Color,
    pub foreground: Color,
    base16: BTreeMap<&'static str, Color>,
}

impl Palette {
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read palette file `{}`", path.display()))?;
        let parsed: WalData = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse palette file `{}`", path.display()))?;

        let background = Color::parse(&parsed.special.background)?;
        let foreground = Color::parse(&parsed.special.foreground)?;
        let cursor = Color::parse(&parsed.special.cursor)?;
        let dark_background = background.luminance() < 0.3;

        let mut base16 = BTreeMap::new();
        base16.insert("base00", background);
        base16.insert("base01", background.lighten(0.05));
        base16.insert("base02", background.lighten(0.11));
        base16.insert("base03", background.lighten(0.2));
        base16.insert("base04", foreground.mix(background, 0.25));
        base16.insert("base05", foreground);
        base16.insert("base06", foreground.lighten(0.08));
        base16.insert("base07", foreground.lighten(0.16));
        base16.insert(
            "base08",
            semantic_accent(
                Color::parse(&parsed.colors.color1)?,
                dark_background,
                350.0,
                0.64,
            ),
        );
        base16.insert(
            "base09",
            semantic_accent(
                Color::parse(&parsed.colors.color9)?,
                dark_background,
                28.0,
                0.70,
            ),
        );
        base16.insert(
            "base0A",
            semantic_accent(
                Color::parse(&parsed.colors.color3)?,
                dark_background,
                55.0,
                0.68,
            ),
        );
        base16.insert(
            "base0B",
            semantic_accent(
                Color::parse(&parsed.colors.color2)?,
                dark_background,
                145.0,
                0.60,
            ),
        );
        base16.insert(
            "base0C",
            semantic_accent(
                Color::parse(&parsed.colors.color6)?,
                dark_background,
                190.0,
                0.66,
            ),
        );
        base16.insert(
            "base0D",
            semantic_accent(
                Color::parse(&parsed.colors.color4)?,
                dark_background,
                220.0,
                0.64,
            ),
        );
        base16.insert(
            "base0E",
            semantic_accent(
                Color::parse(&parsed.colors.color5)?,
                dark_background,
                280.0,
                0.70,
            ),
        );
        base16.insert(
            "base0F",
            semantic_accent(
                Color::parse(&parsed.colors.color11)?,
                dark_background,
                18.0,
                0.74,
            ),
        );

        Ok(Self {
            checksum: parsed
                .checksum
                .unwrap_or_else(|| format!("{}-{}", parsed.wallpaper, background.hex())),
            wallpaper: parsed.wallpaper,
            cursor,
            background,
            foreground,
            base16,
        })
    }

    pub fn color(&self, key: &str) -> Color {
        *self.base16.get(key).expect("missing base16 color")
    }

    pub fn render_context(&self) -> BTreeMap<String, String> {
        let mut ctx = BTreeMap::new();

        for key in [
            "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07",
            "base08", "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F",
        ] {
            let color = self.color(key);
            ctx.insert(format!("{key}-hex"), color.hex());
            ctx.insert(format!("{key}-rgb"), color.rgb_csv());
        }

        ctx.insert("wallpaper".into(), self.wallpaper.clone());
        ctx.insert("cursor-hex".into(), self.cursor.hex());
        ctx.insert("background-hex".into(), self.background.hex());
        ctx.insert("foreground-hex".into(), self.foreground.hex());
        ctx
    }
}

fn semantic_accent(
    color: Color,
    dark_background: bool,
    target_hue: f32,
    target_lightness: f32,
) -> Color {
    let (hue, saturation, lightness) = color.to_hsl();
    let min_saturation = if dark_background { 0.40 } else { 0.34 };
    let adjusted_lightness = if dark_background {
        lightness.max(target_lightness)
    } else {
        lightness.min(target_lightness)
    };
    let adjusted_hue = blend_hue(hue, target_hue, 0.78);

    Color::from_hsl(
        adjusted_hue,
        saturation.max(min_saturation),
        adjusted_lightness,
    )
}

fn blend_hue(source: f32, target: f32, ratio: f32) -> f32 {
    let delta = ((target - source + 540.0).rem_euclid(360.0)) - 180.0;
    (source + delta * ratio).rem_euclid(360.0)
}

pub fn render_template(template: &str, values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .fold(template.to_owned(), |acc, (key, value)| {
            acc.replace(&format!("{{{{{key}}}}}"), value)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_colors() {
        let base = Color { r: 0, g: 0, b: 0 };
        let mixed = base.lighten(0.5);
        assert_eq!(mixed.hex(), "808080");
    }

    #[test]
    fn renders_template_placeholders() {
        let mut values = BTreeMap::new();
        values.insert("base00-hex".into(), "112233".into());
        assert_eq!(
            render_template("color = #{{base00-hex}}", &values),
            "color = #112233"
        );
    }
}
