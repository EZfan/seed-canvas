//! `mandala` CLI — thin shim around `mandala::build()`.

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
    name = "mandala",
    about = "Render the official mandala example template."
)]
struct Cli {
    #[arg(long, default_value = "cosmos")]
    seed: String,
    #[arg(long, default_value = "png")]
    format: String,
    #[arg(long, default_value = "mandala.png")]
    out: PathBuf,
    #[arg(long, default_value = "{}")]
    params: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let template = mandala::build();
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
        size_override: None,
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
