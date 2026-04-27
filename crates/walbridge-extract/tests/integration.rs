//! End-to-end: synthesize an image rich in olive-mud pixels plus a cooler
//! dark cluster, and verify the blacklist ejects the olive and the
//! selected background lands outside the forbidden region.

use image::{ImageBuffer, Rgb};
use tempfile::NamedTempFile;
use walbridge_extract::{color::Srgb, config::Config, extract};

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
