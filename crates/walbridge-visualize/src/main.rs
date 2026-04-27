mod app;

use anyhow::Result;
use app::VisualizerApp;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Visualize the palette walbridge-extract derives from an image"
)]
struct Cli {
    /// Image to extract a palette from. If omitted, use the in-app file picker.
    #[arg(long)]
    image: Option<PathBuf>,
    /// Override config path. Falls back to $XDG_CONFIG_HOME/walbridge/extract.toml.
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("walbridge-visualize"),
        ..Default::default()
    };

    eframe::run_native(
        "walbridge-visualize",
        native_options,
        Box::new(move |_cc| {
            Ok(Box::new(VisualizerApp::new(
                cli.image.clone(),
                cli.config.clone(),
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe: {e}"))
}
