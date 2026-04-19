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

    pub fn darken(self, ratio: f32) -> Self {
        self.mix(Color { r: 0, g: 0, b: 0 }, ratio)
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

    pub fn contrast_ratio(self, other: Color) -> f32 {
        let l1 = self.luminance();
        let l2 = other.luminance();
        let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalPalette {
    pub normal: [Color; 8],
    pub bright: [Color; 8],
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
        let accent_seeds = [
            Color::parse(&parsed.colors.color1)?,
            Color::parse(&parsed.colors.color2)?,
            Color::parse(&parsed.colors.color3)?,
            Color::parse(&parsed.colors.color4)?,
            Color::parse(&parsed.colors.color5)?,
            Color::parse(&parsed.colors.color6)?,
            Color::parse(&parsed.colors.color9)?,
            Color::parse(&parsed.colors.color10)?,
            Color::parse(&parsed.colors.color11)?,
            Color::parse(&parsed.colors.color12)?,
            Color::parse(&parsed.colors.color13)?,
            Color::parse(&parsed.colors.color14)?,
        ];
        let neutral_ramp = derive_neutral_ramp(
            background,
            foreground,
            accent_anchor(&accent_seeds, background),
            dark_background,
        );

        let mut base16 = BTreeMap::new();
        base16.insert("base00", neutral_ramp[0]);
        base16.insert("base01", neutral_ramp[1]);
        base16.insert("base02", neutral_ramp[2]);
        base16.insert("base03", neutral_ramp[3]);
        base16.insert("base04", neutral_ramp[4]);
        base16.insert("base05", neutral_ramp[5]);
        base16.insert("base06", neutral_ramp[6]);
        base16.insert("base07", neutral_ramp[7]);
        base16.insert(
            "base08",
            semantic_accent(
                Color::parse(&parsed.colors.color1)?,
                dark_background,
                350.0,
                0.72,
                0.73,
            ),
        );
        base16.insert(
            "base09",
            semantic_accent(
                Color::parse(&parsed.colors.color9)?,
                dark_background,
                28.0,
                0.78,
                0.72,
            ),
        );
        base16.insert(
            "base0A",
            semantic_accent(
                Color::parse(&parsed.colors.color3)?,
                dark_background,
                55.0,
                0.76,
                0.78,
            ),
        );
        base16.insert(
            "base0B",
            semantic_accent(
                Color::parse(&parsed.colors.color2)?,
                dark_background,
                135.0,
                0.58,
                0.74,
            ),
        );
        base16.insert(
            "base0C",
            semantic_accent(
                Color::parse(&parsed.colors.color6)?,
                dark_background,
                182.0,
                0.60,
                0.74,
            ),
        );
        base16.insert(
            "base0D",
            semantic_accent(
                Color::parse(&parsed.colors.color4)?,
                dark_background,
                218.0,
                0.82,
                0.75,
            ),
        );
        base16.insert(
            "base0E",
            semantic_accent(
                Color::parse(&parsed.colors.color5)?,
                dark_background,
                268.0,
                0.78,
                0.78,
            ),
        );
        base16.insert(
            "base0F",
            semantic_accent(
                Color::parse(&parsed.colors.color11)?,
                dark_background,
                12.0,
                0.58,
                0.80,
            ),
        );

        Ok(Self {
            checksum: parsed
                .checksum
                .unwrap_or_else(|| format!("{}-{}", parsed.wallpaper, background.hex())),
            wallpaper: parsed.wallpaper,
            cursor,
            background: neutral_ramp[0],
            foreground: neutral_ramp[5],
            base16,
        })
    }

    pub fn color(&self, key: &str) -> Color {
        *self.base16.get(key).expect("missing base16 color")
    }

    pub fn render_context(&self) -> BTreeMap<String, String> {
        let mut ctx = BTreeMap::new();
        let terminal = self.terminal_palette();

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
        ctx.insert("terminal-black-hex".into(), terminal.normal[0].hex());
        ctx.insert("terminal-red-hex".into(), terminal.normal[1].hex());
        ctx.insert("terminal-green-hex".into(), terminal.normal[2].hex());
        ctx.insert("terminal-yellow-hex".into(), terminal.normal[3].hex());
        ctx.insert("terminal-blue-hex".into(), terminal.normal[4].hex());
        ctx.insert("terminal-magenta-hex".into(), terminal.normal[5].hex());
        ctx.insert("terminal-cyan-hex".into(), terminal.normal[6].hex());
        ctx.insert("terminal-white-hex".into(), terminal.normal[7].hex());
        ctx.insert("terminal-bright-black-hex".into(), terminal.bright[0].hex());
        ctx.insert("terminal-bright-red-hex".into(), terminal.bright[1].hex());
        ctx.insert("terminal-bright-green-hex".into(), terminal.bright[2].hex());
        ctx.insert(
            "terminal-bright-yellow-hex".into(),
            terminal.bright[3].hex(),
        );
        ctx.insert("terminal-bright-blue-hex".into(), terminal.bright[4].hex());
        ctx.insert(
            "terminal-bright-magenta-hex".into(),
            terminal.bright[5].hex(),
        );
        ctx.insert("terminal-bright-cyan-hex".into(), terminal.bright[6].hex());
        ctx.insert("terminal-bright-white-hex".into(), terminal.bright[7].hex());
        ctx
    }

    pub fn terminal_palette(&self) -> TerminalPalette {
        TerminalPalette {
            normal: [
                self.color("base01"),
                self.color("base08"),
                self.color("base0B"),
                self.color("base0A"),
                self.color("base0D"),
                self.color("base0E"),
                self.color("base0C"),
                self.color("base04").mix(self.color("base05"), 0.35),
            ],
            bright: [
                self.color("base03"),
                terminal_bright(self.color("base08")),
                terminal_bright(self.color("base0B")),
                terminal_bright(self.color("base0A")),
                terminal_bright(self.color("base0D")),
                terminal_bright(self.color("base0E")),
                terminal_bright(self.color("base0C")),
                self.color("base06"),
            ],
        }
    }
}

fn semantic_accent(
    color: Color,
    dark_background: bool,
    target_hue: f32,
    target_saturation: f32,
    target_lightness: f32,
) -> Color {
    let (hue, saturation, lightness) = color.to_hsl();
    let hue_ratio = if saturation < 0.18 { 0.96 } else { 0.86 };
    let adjusted_hue = blend_hue(hue, target_hue, hue_ratio);
    let adjusted_saturation =
        mix_value(saturation, target_saturation, 0.68).clamp(target_saturation * 0.88, 0.96);
    let adjusted_lightness = if dark_background {
        mix_value(lightness, target_lightness, 0.82)
            .clamp(target_lightness - 0.06, target_lightness + 0.03)
    } else {
        mix_value(lightness, target_lightness, 0.74)
            .clamp(target_lightness - 0.03, target_lightness + 0.08)
    };

    Color::from_hsl(adjusted_hue, adjusted_saturation, adjusted_lightness)
}

fn blend_hue(source: f32, target: f32, ratio: f32) -> f32 {
    let delta = ((target - source + 540.0).rem_euclid(360.0)) - 180.0;
    (source + delta * ratio).rem_euclid(360.0)
}

fn mix_value(source: f32, target: f32, ratio: f32) -> f32 {
    (source * (1.0 - ratio)) + (target * ratio)
}

fn accent_anchor(colors: &[Color], fallback: Color) -> Color {
    colors
        .iter()
        .copied()
        .max_by(|left, right| color_weight(*left).total_cmp(&color_weight(*right)))
        .unwrap_or(fallback)
}

fn color_weight(color: Color) -> f32 {
    let (_, saturation, lightness) = color.to_hsl();
    saturation * 1.8 + lightness * 0.35
}

fn harmonized_surface(
    background: Color,
    hue: f32,
    saturation: f32,
    lightness: f32,
    ratio: f32,
) -> Color {
    background.mix(Color::from_hsl(hue, saturation, lightness), ratio)
}

fn derive_neutral_ramp(
    background: Color,
    foreground: Color,
    anchor: Color,
    dark_background: bool,
) -> [Color; 8] {
    let (bg_h, bg_s, bg_l) = background.to_hsl();
    let (anchor_h, anchor_s, _) = anchor.to_hsl();
    let neutral_hue = blend_hue(bg_h, anchor_h, if dark_background { 0.68 } else { 0.42 });
    let neutral_saturation = if dark_background {
        mix_value(bg_s, anchor_s.min(0.48), 0.28).clamp(0.10, 0.24)
    } else {
        mix_value(bg_s, anchor_s.min(0.30), 0.18).clamp(0.04, 0.12)
    };

    if dark_background {
        let base00 = harmonized_surface(
            background,
            neutral_hue,
            neutral_saturation,
            bg_l.max(0.08).min(0.11),
            0.36,
        )
        .darken(0.03);
        let base01 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.14, 0.52);
        let base02 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.19, 0.68);
        let base03 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.31, 0.84);
        let target_foreground = Color::from_hsl(
            neutral_hue,
            (neutral_saturation * 0.48).clamp(0.05, 0.14),
            0.84,
        );
        let mut base05 = foreground.mix(target_foreground, 0.44);
        if base05.contrast_ratio(base00) < 7.2 {
            base05 = target_foreground;
        }

        [
            base00,
            base01,
            base02,
            base03,
            base03.mix(base05, 0.28),
            base05,
            base05.lighten(0.08),
            base05.lighten(0.16),
        ]
    } else {
        let base00 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.97, 0.52);
        let base01 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.93, 0.48);
        let base02 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.88, 0.42);
        let base03 = harmonized_surface(background, neutral_hue, neutral_saturation, 0.72, 0.30);
        let target_foreground = Color::from_hsl(
            neutral_hue,
            (neutral_saturation * 0.72).clamp(0.05, 0.18),
            0.24,
        );
        let base05 = foreground.mix(target_foreground, 0.36);

        [
            base00,
            base01,
            base02,
            base03,
            base03.mix(base05, 0.34),
            base05,
            base05.darken(0.08),
            base05.darken(0.16),
        ]
    }
}

fn terminal_bright(color: Color) -> Color {
    let (hue, saturation, lightness) = color.to_hsl();
    Color::from_hsl(
        hue,
        (saturation * 1.08).clamp(0.52, 0.96),
        (lightness + 0.05).clamp(0.0, 0.86),
    )
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
