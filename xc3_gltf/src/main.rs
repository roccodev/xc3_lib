use std::path::Path;

use anyhow::Context;
use clap::Parser;
use xc3_model::{gltf::GltfFile, load_model, load_model_legacy, shader_database::ShaderDatabase};

/// Convert wimdo and wismhd models to glTF for
/// Xenoblade X, Xenoblade 1 DE, Xenoblade 2, and Xenoblade 3.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// The input wimdo, pcmdo, camdo, or wismhd file.
    input: String,
    /// The output gltf file.
    /// Images will be saved to the same directory as the output.
    output: String,
    /// The shader JSON database generated by xc3_shader.
    database: Option<String>,
}

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .init()
        .unwrap();

    let cli = Cli::parse();

    let start = std::time::Instant::now();

    let database = cli
        .database
        .map(|p| ShaderDatabase::from_file(&p).with_context(|| format!("{p:?}")))
        .transpose()?;

    let roots = match Path::new(&cli.input).extension().unwrap().to_str().unwrap() {
        "wimdo" => {
            let root = load_model(&cli.input, database.as_ref())
                .with_context(|| format!("failed to load .wimdo model {:?}", cli.input))?;
            Ok(vec![root])
        }
        "pcmdo" => {
            let root = load_model(&cli.input, database.as_ref())
                .with_context(|| format!("failed to load .pcmdo model {:?}", cli.input))?;
            Ok(vec![root])
        }
        "camdo" => {
            let root = load_model_legacy(&cli.input);
            Ok(vec![root])
        }
        "wismhd" => xc3_model::load_map(&cli.input, database.as_ref())
            .with_context(|| format!("failed to load .wismhd map {:?}", cli.input)),
        e => Err(anyhow::anyhow!("unsupported extension {e}")),
    }?;

    let name = std::path::Path::new(&cli.output)
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    if let Some(parent) = Path::new(&cli.output).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {parent:?}"))?;
    }

    let file = GltfFile::new(&name, &roots).with_context(|| "failed to create glTF file")?;
    file.save(&cli.output)
        .with_context(|| format!("failed to save glTF file to {:?}", &cli.output))?;

    println!("Converted {} roots in {:?}", roots.len(), start.elapsed());
    Ok(())
}
