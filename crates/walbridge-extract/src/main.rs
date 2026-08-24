use anyhow::Result;
use clap::Parser;
use std::{path::PathBuf, process::ExitCode};
use walbridge_extract::{config::Config, extract, output};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Extract a palette from an image, emit pywal-compatible colors.json plus a richer palette.json"
)]
struct Cli {
    /// Wallpaper / source image.
    #[arg(long)]
    image: PathBuf,
    /// Where to write pywal-compatible colors.json.
    #[arg(long, default_value = "~/.cache/wal/colors.json")]
    colors_out: String,
    /// Where to write the richer palette.json.
    #[arg(long, default_value = "~/.cache/wal/palette.json")]
    palette_out: String,
    /// Optionally write a deterministic Tint-compatible Base16 scheme.
    #[arg(long)]
    base16_out: Option<String>,
    /// Override config path. Falls back to $XDG_CONFIG_HOME/walbridge/extract.toml.
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config.or_else(Config::default_config_path);
    let cfg = Config::load(config_path.as_deref())?;

    let extraction = extract::extract(&cli.image, &cfg)?;

    let colors_out = expand_tilde(&cli.colors_out);
    let palette_out = expand_tilde(&cli.palette_out);

    output::write_colors_json(&colors_out, &extraction)?;
    output::write_palette_json(&palette_out, &extraction)?;
    if let Some(base16_out) = cli.base16_out.as_deref() {
        let palette = walbridge::palette::Palette::from_file(&colors_out)?;
        output::write_base16_yaml(&expand_tilde(base16_out), &palette)?;
    }

    let bg = extraction.background.srgb;
    let fg = extraction.foreground.srgb;
    println!(
        "wrote {} ({} clusters, {} blacklisted)  bg={} fg={}",
        colors_out.display(),
        extraction.clusters.len(),
        extraction
            .clusters
            .iter()
            .filter(|c| c.rejected_by.is_some())
            .count(),
        bg.hex_with_hash(),
        fg.hex_with_hash(),
    );
    Ok(())
}

fn expand_tilde(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(input)
}
