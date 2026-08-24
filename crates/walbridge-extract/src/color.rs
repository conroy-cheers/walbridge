//! sRGB and OKLab color types plus lossless conversions.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Srgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Srgb {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn parse_hex(input: &str) -> anyhow::Result<Self> {
        let trimmed = input.trim().trim_start_matches('#');
        anyhow::ensure!(trimmed.len() == 6, "invalid color `{input}`");
        Ok(Self {
            r: u8::from_str_radix(&trimmed[0..2], 16)?,
            g: u8::from_str_radix(&trimmed[2..4], 16)?,
            b: u8::from_str_radix(&trimmed[4..6], 16)?,
        })
    }

    pub fn hex_with_hash(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn to_oklab(self) -> Oklab {
        let (lr, lg, lb) = (
            srgb_to_linear(self.r as f32 / 255.0),
            srgb_to_linear(self.g as f32 / 255.0),
            srgb_to_linear(self.b as f32 / 255.0),
        );
        linear_srgb_to_oklab(lr, lg, lb)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Oklab {
    pub fn new(l: f32, a: f32, b: f32) -> Self {
        Self { l, a, b }
    }

    pub fn chroma(self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    /// Hue in degrees, 0..360, atan2(b, a).
    pub fn hue_deg(self) -> f32 {
        let h = self.b.atan2(self.a).to_degrees();
        if h < 0.0 {
            h + 360.0
        } else {
            h
        }
    }

    /// Squared Euclidean distance. Good enough for k-means.
    pub fn dist_sq(self, other: Oklab) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }

    pub fn to_srgb(self) -> Srgb {
        let (lr, lg, lb) = oklab_to_linear_srgb(self.l, self.a, self.b);
        // Clamp to gamut. OKLab is wider than sRGB; out-of-gamut rotations
        // get clipped rather than silently projected, which is fine here —
        // inputs come from sRGB clusters, so clipping only bites when we
        // rotate hue aggressively in `assign`.
        Srgb {
            r: (linear_to_srgb(lr).clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (linear_to_srgb(lg).clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (linear_to_srgb(lb).clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }

    pub fn with_lightness(self, l: f32) -> Self {
        Self {
            l,
            a: self.a,
            b: self.b,
        }
    }

    /// Rotate hue to `target_deg`, preserving L and chroma.
    pub fn with_hue(self, target_deg: f32) -> Self {
        let c = self.chroma();
        let theta = target_deg.to_radians();
        Self {
            l: self.l,
            a: c * theta.cos(),
            b: c * theta.sin(),
        }
    }

    pub fn with_chroma(self, new_chroma: f32) -> Self {
        let c = self.chroma();
        if c < f32::EPSILON {
            // No hue defined; bias toward a-axis so scaling has an effect.
            return Self {
                l: self.l,
                a: new_chroma,
                b: 0.0,
            };
        }
        let k = new_chroma / c;
        Self {
            l: self.l,
            a: self.a * k,
            b: self.b * k,
        }
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn linear_srgb_to_oklab(r: f32, g: f32, b: f32) -> Oklab {
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();
    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

fn oklab_to_linear_srgb(ll: f32, aa: f32, bb: f32) -> (f32, f32, f32) {
    let l_ = ll + 0.3963377774 * aa + 0.2158037573 * bb;
    let m_ = ll - 0.1055613458 * aa - 0.0638541728 * bb;
    let s_ = ll - 0.0894841775 * aa - 1.2914855480 * bb;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_roundtrip_via_oklab() {
        for &hex in &["#000000", "#ffffff", "#ff0000", "#19190a", "#2a8fd3"] {
            let c = Srgb::parse_hex(hex).unwrap();
            let back = c.to_oklab().to_srgb();
            // Allow ±1 per channel for the sRGB quantization roundtrip.
            assert!((c.r as i32 - back.r as i32).abs() <= 1, "{hex} r drifted");
            assert!((c.g as i32 - back.g as i32).abs() <= 1, "{hex} g drifted");
            assert!((c.b as i32 - back.b as i32).abs() <= 1, "{hex} b drifted");
        }
    }

    #[test]
    fn olive_mud_sits_in_expected_oklab_region() {
        // #19190a — the color from the green wallpaper that started this.
        let lab = Srgb::parse_hex("#19190a").unwrap().to_oklab();
        assert!(lab.l < 0.35, "expected dark: L={}", lab.l);
        assert!(lab.b > 0.01, "expected yellow-tinted (b>0): b={}", lab.b);
    }

    #[test]
    fn pure_red_has_expected_hue() {
        let h = Srgb::parse_hex("#ff0000").unwrap().to_oklab().hue_deg();
        assert!((h - 29.23).abs() < 1.0, "red hue was {h}");
    }
}
