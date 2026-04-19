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

        let mut base16 = BTreeMap::new();
        base16.insert("base00", background);
        base16.insert("base01", background.lighten(0.05));
        base16.insert("base02", background.lighten(0.11));
        base16.insert("base03", background.lighten(0.2));
        base16.insert("base04", foreground.mix(background, 0.25));
        base16.insert("base05", foreground);
        base16.insert("base06", foreground.lighten(0.08));
        base16.insert("base07", foreground.lighten(0.16));
        base16.insert("base08", Color::parse(&parsed.colors.color1)?);
        base16.insert("base09", Color::parse(&parsed.colors.color9)?);
        base16.insert("base0A", Color::parse(&parsed.colors.color3)?);
        base16.insert("base0B", Color::parse(&parsed.colors.color2)?);
        base16.insert("base0C", Color::parse(&parsed.colors.color6)?);
        base16.insert("base0D", Color::parse(&parsed.colors.color4)?);
        base16.insert("base0E", Color::parse(&parsed.colors.color5)?);
        base16.insert("base0F", Color::parse(&parsed.colors.color11)?);

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
