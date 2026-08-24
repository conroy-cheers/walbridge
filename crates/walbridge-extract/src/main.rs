use anyhow::Result;
use clap::Parser;
use std::{path::PathBuf, process::ExitCode};
use walbridge_extract::{config::Config, extract, output};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Extract a palette from an image and write the selected output formats"
)]
struct Cli {
    /// Wallpaper / source image.
    #[arg(long)]
    image: PathBuf,
    /// Where to write pywal-compatible colors.json. Used with palette.json by default when no output is selected.
    #[arg(long)]
    colors_out: Option<String>,
    /// Where to write the richer palette.json. Used with colors.json by default when no output is selected.
    #[arg(long)]
    palette_out: Option<String>,
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

    let use_default_outputs =
        cli.colors_out.is_none() && cli.palette_out.is_none() && cli.base16_out.is_none();
    let colors_out = cli
        .colors_out
        .as_deref()
        .map(expand_tilde)
        .or_else(|| use_default_outputs.then(|| expand_tilde("~/.cache/wal/colors.json")));
    let palette_out = cli
        .palette_out
        .as_deref()
        .map(expand_tilde)
        .or_else(|| use_default_outputs.then(|| expand_tilde("~/.cache/wal/palette.json")));

    if let Some(colors_out) = colors_out.as_deref() {
        output::write_colors_json(colors_out, &extraction)?;
    }
    if let Some(palette_out) = palette_out.as_deref() {
        output::write_palette_json(palette_out, &extraction)?;
    }
    if let Some(base16_out) = cli.base16_out.as_deref() {
        let palette = output::palette(&extraction);
        output::write_base16_yaml(&expand_tilde(base16_out), &palette)?;
    }

    let bg = extraction.background.srgb;
    let fg = extraction.foreground.srgb;
    println!(
        "extracted palette ({} clusters, {} blacklisted)  bg={} fg={}",
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
