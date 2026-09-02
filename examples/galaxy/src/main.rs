//! `galaxy` CLI — renders the official galaxy example template.
//!
//! The interesting work lives in `galaxy::build()`. This binary just wires
//! up the CLI flags and writes the encoded bytes to disk.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use seed_canvas_adapter_server::ServerAdapter;
use seed_canvas_adapter_svg::SvgAdapter;
use seed_canvas_core::adapter::{Adapter, AdapterKind, AdapterRegistry};
use seed_canvas_core::render::{render, RenderRequest};
use seed_canvas_core::surface::OutputFormat;
use seed_canvas_core::Seed;

#[derive(Parser, Debug)]
#[command(
    name = "galaxy",
    about = "Render the official galaxy example template."
)]
struct Cli {
    /// Deterministic seed string. Identical inputs always produce identical output.
    #[arg(long, default_value = "cosmos")]
    seed: String,

    /// Output format: png (default) or svg.
    #[arg(long, default_value = "png")]
    format: String,

    /// Output file path. The file is created (or overwritten) with the
    /// encoded artwork.
    #[arg(long, default_value = "galaxy.png")]
    out: PathBuf,

    /// JSON-encoded params, e.g. `{"count": 1500}`.
    #[arg(long, default_value = "{}")]
    params: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let template = galaxy::build();
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(ServerAdapter::new()) as Arc<dyn Adapter>);
    registry.register(Arc::new(SvgAdapter::new()) as Arc<dyn Adapter>);

    let (kind, format) = match cli.format.as_str() {
        "svg" => (AdapterKind::Svg, OutputFormat::Svg),
        _ => (AdapterKind::Server, OutputFormat::Png),
    };

    let params: serde_json::Value = serde_json::from_str(&cli.params)
        .map_err(|e| format!("--params must be valid JSON: {e}"))?;

    let request = RenderRequest {
        seed: Seed::from_string(cli.seed),
        params,
        adapter: kind,
        format,
    };

    let output = render(&template, &request, &registry)?;
    std::fs::write(&cli.out, &output.bytes)?;
    eprintln!(
        "wrote {} bytes ({:?}) to {} (content_hash={})",
        output.bytes.len(),
        output.format,
        cli.out.display(),
        &output.content_hash[..16],
    );
    Ok(())
}
