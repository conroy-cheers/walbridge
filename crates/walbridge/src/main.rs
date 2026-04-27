mod adapters;
mod inventory;
mod palette;

use adapters::{AdapterStatus, ApplyContext};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use inventory::stylix_inventory;
use palette::Palette;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Bridge pywal colors into app configs at runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Apply {
        #[arg(long)]
        palette: PathBuf,
        #[arg(long)]
        state: Option<PathBuf>,
    },
    Status {
        #[arg(long)]
        state: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Inventory {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusManifest {
    schema_version: u32,
    generated_at_unix: u64,
    palette_path: String,
    palette_checksum: String,
    wallpaper: String,
    adapters: Vec<AdapterStatus>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Apply { palette, state } => apply_command(&palette, state.as_deref()),
        Command::Status { state, json } => status_command(state.as_deref(), json),
        Command::Inventory { json } => inventory_command(json),
    }
}

fn apply_command(palette_path: &Path, state_override: Option<&Path>) -> Result<()> {
    let palette = Palette::from_file(palette_path)?;
    let context = ApplyContext::from_env()?;
    let adapters = adapters::apply_all(&context, &palette);
    let state_path = state_override
        .map(PathBuf::from)
        .unwrap_or_else(default_state_path);

    let manifest = StatusManifest {
        schema_version: 1,
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before UNIX_EPOCH")?
            .as_secs(),
        palette_path: palette_path.display().to_string(),
        palette_checksum: palette.checksum.clone(),
        wallpaper: palette.wallpaper.clone(),
        adapters,
    };

    write_status(&state_path, &manifest)?;

    for adapter in &manifest.adapters {
        let status = if adapter.success { "ok" } else { "failed" };
        println!("{}: {} ({:?})", adapter.name, status, adapter.apply_mode);
        if let Some(note) = &adapter.note {
            println!("  note: {note}");
        }
        if let Some(error) = &adapter.error {
            println!("  error: {error}");
        }
    }

    if manifest.adapters.iter().any(|adapter| !adapter.success) {
        anyhow::bail!("one or more adapters failed");
    }

    Ok(())
}

fn status_command(state_override: Option<&Path>, json: bool) -> Result<()> {
    let state_path = state_override
        .map(PathBuf::from)
        .unwrap_or_else(default_state_path);
    let manifest: StatusManifest = serde_json::from_str(
        &fs::read_to_string(&state_path)
            .with_context(|| format!("failed to read `{}`", state_path.display()))?,
    )
    .with_context(|| format!("failed to parse `{}`", state_path.display()))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
        return Ok(());
    }

    println!("palette_checksum={}", manifest.palette_checksum);
    println!("wallpaper={}", manifest.wallpaper);
    println!("generated_at_unix={}", manifest.generated_at_unix);
    for adapter in manifest.adapters {
        let status = if adapter.success { "ok" } else { "failed" };
        println!(
            "{}: status={} apply_mode={:?}",
            adapter.name, status, adapter.apply_mode
        );
        if let Some(note) = adapter.note {
            println!("  note: {note}");
        }
        if let Some(error) = adapter.error {
            println!("  error: {error}");
        }
    }

    Ok(())
}

fn inventory_command(json: bool) -> Result<()> {
    let inventory = stylix_inventory();

    if json {
        println!("{}", serde_json::to_string_pretty(&inventory)?);
        return Ok(());
    }

    for target in inventory {
        println!(
            "{}: scope={:?} status={:?}",
            target.name, target.scope, target.status
        );
        println!("  reason: {}", target.reason);
    }

    Ok(())
}

fn default_state_path() -> PathBuf {
    let state_root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));
    state_root.join("walbridge/status.json")
}

fn write_status(path: &Path, manifest: &StatusManifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }

    fs::write(path, serde_json::to_vec_pretty(manifest)?)
        .with_context(|| format!("failed to write `{}`", path.display()))
}
