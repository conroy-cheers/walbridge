//! End-to-end: synthesize an image rich in olive-mud pixels plus a cooler
//! dark cluster, and verify the blacklist ejects the olive and the
//! selected background lands outside the forbidden region.

use image::{ImageBuffer, Rgb};
use std::process::Command;
use tempfile::NamedTempFile;
use walbridge_extract::{color::Srgb, config::Config, extract, output};

fn write_test_image(path: &std::path::Path) {
    // 60% olive-mud, 30% cool dark navy, 10% light neutral.
    let w = 200u32;
    let h = 200u32;
    let mut buf: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(w, h);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        let ratio = (y as f32 * w as f32 + x as f32) / (w * h) as f32;
        let (r, g, b) = if ratio < 0.60 {
            (0x19, 0x19, 0x0a) // olive mud
        } else if ratio < 0.90 {
            (0x0d, 0x16, 0x24) // cool dark navy
        } else {
            (0xe8, 0xe8, 0xe4) // light neutral
        };
        *px = Rgb([r, g, b]);
    }
    buf.save(path).expect("save test image");
}

#[test]
fn background_escapes_olive_mud_region() {
    let tmp = NamedTempFile::new().unwrap();
    let img_path = tmp.path().with_extension("png");
    write_test_image(&img_path);

    let cfg = Config::default();
    let out = extract::extract(&img_path, &cfg).expect("extract");

    for region in &cfg.blacklist {
        assert!(
            !region.contains(out.background.oklab),
            "background {:?} landed inside forbidden region `{}`",
            out.background.srgb,
            region.name,
        );
    }

    assert!(
        out.background.oklab.l <= 0.30,
        "expected dark background, got L={}",
        out.background.oklab.l
    );

    assert!(
        out.blacklist_applied.iter().any(|n| n == "olive mud"),
        "expected olive mud to have been filtered, applied={:?}",
        out.blacklist_applied
    );

    std::fs::remove_file(&img_path).ok();
}

#[test]
fn extraction_is_deterministic_for_same_image() {
    let tmp = NamedTempFile::new().unwrap();
    let img_path = tmp.path().with_extension("png");
    write_test_image(&img_path);

    let cfg = Config::default();
    let a = extract::extract(&img_path, &cfg).unwrap();
    let b = extract::extract(&img_path, &cfg).unwrap();
    assert_eq!(a.background.srgb, b.background.srgb);
    assert_eq!(a.foreground.srgb, b.foreground.srgb);
    for (x, y) in a.accents.iter().zip(b.accents.iter()) {
        assert_eq!(x.srgb, y.srgb);
    }

    std::fs::remove_file(&img_path).ok();
}

#[test]
fn base16_output_is_complete_and_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let img_path = tmp.path().join("image.png");
    let first_path = tmp.path().join("first.yaml");
    let second_path = tmp.path().join("second.yaml");
    write_test_image(&img_path);

    let extraction = extract::extract(&img_path, &Config::default()).unwrap();
    let palette = output::palette(&extraction);
    output::write_base16_yaml(&first_path, &palette).unwrap();
    output::write_base16_yaml(&second_path, &palette).unwrap();

    let first = std::fs::read_to_string(first_path).unwrap();
    let second = std::fs::read_to_string(second_path).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("system: \"base16\""));
    assert!(first.contains("variant: \"dark\""));
    for index in 0..16 {
        assert!(
            first.contains(&format!("  base{index:02X}: \"#")),
            "missing Base16 slot base{index:02X}\n{first}",
        );
    }
}

#[test]
fn stylix_palette_generator_writes_the_native_protocol() {
    let tmp = tempfile::tempdir().unwrap();
    let image_path = tmp.path().join("image.png");
    let output_path = tmp.path().join("palette.json");
    write_test_image(&image_path);

    let status = Command::new(env!("CARGO_BIN_EXE_palette-generator"))
        .args(["dark"])
        .arg(&image_path)
        .arg(&output_path)
        .status()
        .unwrap();
    assert!(status.success());

    let generated: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&std::fs::read(output_path).unwrap()).unwrap();
    assert_eq!(generated.len(), 16);
    for index in 0..16 {
        let key = format!("base{index:02X}");
        let color = generated.get(&key).unwrap().as_str().unwrap();
        assert_eq!(color.len(), 6, "invalid {key}: {color}");
        assert!(color.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn empty_blacklist_uses_olive_if_dominant() {
    let tmp = NamedTempFile::new().unwrap();
    let img_path = tmp.path().with_extension("png");
    write_test_image(&img_path);

    let mut cfg = Config::default();
    cfg.blacklist.clear();
    let out = extract::extract(&img_path, &cfg).unwrap();

    let lab = out.background.oklab;
    let olive = Srgb::parse_hex("#19190a").unwrap().to_oklab();
    let d = lab.dist_sq(olive).sqrt();
    assert!(
        d < 0.05,
        "without blacklist, expected bg near olive ({:?}); got {:?} dist={}",
        olive,
        lab,
        d,
    );

    std::fs::remove_file(&img_path).ok();
}
