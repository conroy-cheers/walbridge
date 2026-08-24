use anyhow::{bail, Context, Result};
use std::{path::PathBuf, process::ExitCode};
use walbridge_extract::{config::Config, extract, output};

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<std::ffi::OsString>) -> Result<()> {
    let [polarity, image, output_path]: [std::ffi::OsString; 3] = arguments
        .try_into()
        .map_err(|_| anyhow::anyhow!("usage: palette-generator <polarity> <image> <output>"))?;
    let polarity = polarity
        .into_string()
        .map_err(|_| anyhow::anyhow!("polarity is not valid UTF-8"))?;
    if polarity != "dark" && polarity != "either" {
        bail!("walbridge palette generation does not support `{polarity}` polarity");
    }

    let image = PathBuf::from(image);
    let output_path = PathBuf::from(output_path);
    let extraction = extract::extract(&image, &Config::default())
        .with_context(|| format!("failed to generate a palette from `{}`", image.display()))?;
    output::write_stylix_json(&output_path, &output::palette(&extraction))
}
