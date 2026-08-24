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
    terminal: TerminalPalette,
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

        let terminal = build_terminal_palette(&base16, neutral_ramp[0], neutral_ramp[5]);

        Ok(Self {
            checksum: parsed
                .checksum
                .unwrap_or_else(|| format!("{}-{}", parsed.wallpaper, background.hex())),
            wallpaper: parsed.wallpaper,
            cursor,
            background: neutral_ramp[0],
            foreground: neutral_ramp[5],
            terminal,
            base16,
        })
    }

    pub fn color(&self, key: &str) -> Color {
        *self.base16.get(key).expect("missing base16 color")
    }

    pub fn is_dark(&self) -> bool {
        self.background.luminance() < 0.3
    }

    pub fn base16_colors(&self) -> impl Iterator<Item = (&'static str, Color)> + '_ {
        self.base16.iter().map(|(name, color)| (*name, *color))
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
        self.terminal
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
        let shadow_hue = blend_hue(bg_h, anchor_h, 0.84);
        let shadow_saturation = mix_value(bg_s, anchor_s.min(0.34), 0.34).clamp(0.07, 0.16);
        let shadow_lightness = bg_l.clamp(0.055, 0.072);
        let shadow_background = Color::from_hsl(shadow_hue, shadow_saturation, shadow_lightness);

        let base00 = shadow_background;
        let base01 = harmonized_surface(
            shadow_background,
            neutral_hue,
            neutral_saturation,
            0.14,
            0.46,
        );
        let base02 = harmonized_surface(
            shadow_background,
            neutral_hue,
            neutral_saturation,
            0.19,
            0.64,
        );
        let base03 = harmonized_surface(
            shadow_background,
            neutral_hue,
            neutral_saturation,
            0.31,
            0.82,
        );
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

fn build_terminal_palette(
    base16: &BTreeMap<&'static str, Color>,
    background: Color,
    foreground: Color,
) -> TerminalPalette {
    let base01 = *base16.get("base01").expect("missing base01");
    let base03 = *base16.get("base03").expect("missing base03");
    let base05 = *base16.get("base05").expect("missing base05");
    let base06 = *base16.get("base06").expect("missing base06");
    let base08 = *base16.get("base08").expect("missing base08");
    let base0a = *base16.get("base0A").expect("missing base0A");
    let base0b = *base16.get("base0B").expect("missing base0B");
    let base0c = *base16.get("base0C").expect("missing base0C");
    let base0d = *base16.get("base0D").expect("missing base0D");
    let base0e = *base16.get("base0E").expect("missing base0E");

    let normal = [
        terminal_neutral(base01, base0c, background, 0.08, 0.10),
        terminal_role(base08, 350.0, 0.82, 0.70, background),
        terminal_role(base0b, 132.0, 0.70, 0.72, background),
        terminal_role(base0a, 58.0, 0.78, 0.74, background),
        terminal_role(base0d, 220.0, 0.82, 0.73, background),
        terminal_role(base0e, 282.0, 0.76, 0.74, background),
        terminal_role(base0c, 188.0, 0.72, 0.74, background),
        terminal_neutral(base05, base0d, background, 0.06, 0.84),
    ];

    let bright = [
        terminal_neutral(base03, base0d, background, 0.10, 0.30),
        terminal_bright_role(base08, 350.0, 0.88, 0.77, background),
        terminal_bright_role(base0b, 132.0, 0.76, 0.79, background),
        terminal_bright_role(base0a, 58.0, 0.84, 0.80, background),
        terminal_bright_role(base0d, 220.0, 0.88, 0.80, background),
        terminal_bright_role(base0e, 282.0, 0.84, 0.80, background),
        terminal_bright_role(base0c, 188.0, 0.78, 0.80, background),
        terminal_neutral(base06, base0c, background, 0.04, 0.88),
    ];

    let mut palette = TerminalPalette { normal, bright };
    enforce_terminal_palette_invariants(&mut palette, background, foreground);
    palette
}

fn terminal_role(
    color: Color,
    target_hue: f32,
    target_saturation: f32,
    target_lightness: f32,
    background: Color,
) -> Color {
    let (hue, saturation, lightness) = color.to_hsl();
    let hue_ratio = if saturation < 0.24 { 0.98 } else { 0.92 };
    let candidate = Color::from_hsl(
        blend_hue(hue, target_hue, hue_ratio),
        mix_value(saturation, target_saturation, 0.82).clamp(target_saturation - 0.08, 0.98),
        mix_value(lightness, target_lightness, 0.84)
            .clamp(target_lightness - 0.05, target_lightness + 0.04),
    );

    ensure_contrast(candidate, background, target_lightness + 0.06, 3.4)
}

fn terminal_bright_role(
    color: Color,
    target_hue: f32,
    target_saturation: f32,
    target_lightness: f32,
    background: Color,
) -> Color {
    terminal_bright(terminal_role(
        color,
        target_hue,
        target_saturation,
        target_lightness,
        background,
    ))
}

fn terminal_neutral(
    neutral: Color,
    tint: Color,
    background: Color,
    tint_ratio: f32,
    target_lightness: f32,
) -> Color {
    let tinted = neutral.mix(tint, tint_ratio);
    let (hue, saturation, _) = tinted.to_hsl();
    ensure_contrast(
        Color::from_hsl(hue, saturation.clamp(0.05, 0.18), target_lightness),
        background,
        target_lightness + 0.06,
        3.1,
    )
}

fn ensure_contrast(
    color: Color,
    background: Color,
    fallback_lightness: f32,
    min_contrast: f32,
) -> Color {
    if color.contrast_ratio(background) >= min_contrast {
        return color;
    }

    let (hue, saturation, lightness) = color.to_hsl();
    let lightness = lightness.max(fallback_lightness).clamp(0.0, 0.92);
    Color::from_hsl(hue, saturation, lightness)
}

fn enforce_terminal_palette_invariants(
    palette: &mut TerminalPalette,
    background: Color,
    foreground: Color,
) {
    if palette.normal[0].hex() == background.hex() {
        palette.normal[0] = terminal_neutral(
            palette.normal[0].lighten(0.04),
            palette.normal[6],
            background,
            0.04,
            0.12,
        );
    }

    if palette.bright[0].hex() == palette.normal[0].hex() {
        palette.bright[0] = palette.normal[0].lighten(0.12);
    }

    if palette.normal[7].hex() == foreground.hex() {
        palette.normal[7] = terminal_neutral(foreground, palette.normal[4], background, 0.05, 0.80);
    }

    if palette.bright[7].hex() == palette.normal[7].hex() {
        palette.bright[7] = palette.normal[7].lighten(0.08);
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
    use std::{fs, path::PathBuf};

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

    #[test]
    fn terminal_palette_stays_separated_for_muddy_inputs() {
        let palette = sample_palette();
        let terminal = palette.terminal_palette();

        assert_ne!(terminal.normal[0].hex(), palette.background.hex());
        assert_ne!(terminal.bright[0].hex(), terminal.normal[0].hex());
        assert_ne!(terminal.normal[7].hex(), palette.foreground.hex());
        assert_ne!(terminal.bright[7].hex(), terminal.normal[7].hex());

        for color in terminal.normal[1..7]
            .iter()
            .chain(terminal.bright[1..7].iter())
        {
            let (_, saturation, _) = color.to_hsl();
            assert!(
                saturation >= 0.45,
                "expected saturated terminal role, got {}",
                color.hex()
            );
            assert!(
                color.contrast_ratio(palette.background) >= 3.0,
                "expected terminal role to stand off from background: {}",
                color.hex()
            );
        }
    }

    #[test]
    fn dark_background_avoids_olive_mud() {
        let palette = sample_palette();
        let (hue, saturation, lightness) = palette.background.to_hsl();

        assert_eq!(palette.background.hex(), "0f1314");
        assert!(
            (160.0..=230.0).contains(&hue),
            "expected cool shadow hue, got {hue}"
        );
        assert!(
            saturation <= 0.18,
            "expected restrained background saturation, got {saturation}"
        );
        assert!(
            lightness <= 0.08,
            "expected terminal background to stay dark, got {lightness}"
        );
    }

    #[test]
    fn render_context_includes_terminal_role_keys() {
        let palette = sample_palette();
        let ctx = palette.render_context();

        for key in [
            "terminal-black-hex",
            "terminal-red-hex",
            "terminal-green-hex",
            "terminal-yellow-hex",
            "terminal-blue-hex",
            "terminal-magenta-hex",
            "terminal-cyan-hex",
            "terminal-white-hex",
            "terminal-bright-black-hex",
            "terminal-bright-red-hex",
            "terminal-bright-green-hex",
            "terminal-bright-yellow-hex",
            "terminal-bright-blue-hex",
            "terminal-bright-magenta-hex",
            "terminal-bright-cyan-hex",
            "terminal-bright-white-hex",
        ] {
            assert!(ctx.contains_key(key), "missing render context key {key}");
        }
    }

    fn sample_palette() -> Palette {
        let json = r##"{
            "checksum": "sample",
            "wallpaper": "/tmp/green.jpg",
            "special": {
                "background": "#19190a",
                "foreground": "#c5c5c1",
                "cursor": "#c5c5c1"
            },
            "colors": {
                "color0": "#19190a",
                "color1": "#7d7885",
                "color2": "#7e88a2",
                "color3": "#818dad",
                "color4": "#8c92a4",
                "color5": "#8794b1",
                "color6": "#9a9daf",
                "color7": "#98988e",
                "color8": "#6e6e59",
                "color9": "#A7A1B2",
                "color10": "#A8B6D9",
                "color11": "#ACBDE7",
                "color12": "#BBC3DB",
                "color13": "#B4C6EC",
                "color14": "#CED2EA",
                "color15": "#c5c5c1"
            }
        }"##;

        let path = unique_test_path("walbridge-palette-sample.json");
        fs::write(&path, json).expect("failed to write sample palette");
        let palette = Palette::from_file(&path).expect("failed to parse sample palette");
        let _ = fs::remove_file(path);
        palette
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock drift")
            .as_nanos();
        std::env::temp_dir().join(format!("{nanos}-{name}"))
    }
}
