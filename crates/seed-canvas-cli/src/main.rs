//! Command-line interface for seed-canvas.
//!
//! Every subcommand is documented inline. Run `seed-canvas --help` to see
//! the full list, or `seed-canvas <command> --help` for command-specific
//! details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use seed_canvas_adapter_server::ServerAdapter;
use seed_canvas_adapter_svg::SvgAdapter;
use seed_canvas_core::adapter::{Adapter, AdapterKind, AdapterRegistry};
use seed_canvas_core::render::{render, RenderRequest};
use seed_canvas_core::surface::OutputFormat;
use seed_canvas_core::Seed;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "seed-canvas",
    version,
    about = "Deterministic generative art platform — seed is the artwork.",
    long_about = "Seed-canvas is an open-source, self-hostable platform for deterministic generative art.\n\
                  The same seed + template + params always produces the same artwork, on every\n\
                  platform, in every format. Run `seed-canvas doctor` to verify your environment."
)]
struct Cli {
    /// Verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize a new seed-canvas workspace in the current directory.
    Init {
        /// Directory to create the workspace in. Defaults to `./gallery`.
        #[arg(default_value = "gallery")]
        dir: PathBuf,
    },

    /// Render a single artwork from a seed + template + params.
    Render {
        /// Template identifier (e.g. `galaxy`).
        #[arg(short, long)]
        template: String,

        /// Deterministic seed string.
        #[arg(short, long)]
        seed: String,

        /// Output format.
        #[arg(short, long, value_enum, default_value_t = FormatArg::Png)]
        format: FormatArg,

        /// Output file path.
        #[arg(short, long)]
        out: PathBuf,

        /// JSON-encoded params.
        #[arg(long, default_value = "{}")]
        params: String,
    },

    /// Render an artwork and write a share-friendly canonical URL.
    Share {
        #[arg(short, long)]
        template: String,
        #[arg(short, long)]
        seed: String,
        #[arg(long, default_value = "out.png")]
        out: PathBuf,
        #[arg(long, default_value = "{}")]
        params: String,
        /// Public URL prefix the share link is rooted under.
        #[arg(long, default_value = "https://art.example.com")]
        host: String,
    },

    /// List installed templates.
    List,

    /// Generate a fresh random seed and print it. Useful for sharing new work.
    Random,

    /// Re-render a previously created artwork and verify the content hash.
    /// Useful for catching determinism regressions.
    Verify {
        #[arg(short, long)]
        template: String,
        #[arg(short, long)]
        seed: String,
        /// Optional JSON file holding the expected content hash.
        #[arg(long)]
        expected_hash: Option<PathBuf>,
    },

    /// Print environment diagnostics (toolchain, adapters, gallery state).
    Doctor,

    /// Show the canonical share URL for a seed/template pair.
    Url {
        #[arg(short, long)]
        template: String,
        #[arg(short, long)]
        seed: String,
        #[arg(long, default_value = "https://art.example.com")]
        host: String,
    },

    /// Run the self-hosted HTTP gallery server. Open the printed URL in
    /// a browser to browse the gallery.
    Serve {
        /// Address to bind. Use `0.0.0.0:8080` to accept LAN traffic.
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
        /// Workspace root (defaults to the current directory).
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum FormatArg {
    Png,
    Svg,
    Json,
}

impl From<FormatArg> for OutputFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Png => Self::Png,
            FormatArg::Svg => Self::Svg,
            FormatArg::Json => Self::Json,
        }
    }
}

impl From<FormatArg> for AdapterKind {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Png => Self::Server,
            FormatArg::Svg => Self::Svg,
            FormatArg::Json => Self::Server, // server adapter supports JSON dump
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(ServerAdapter::new()) as Arc<dyn Adapter>);
    registry.register(Arc::new(SvgAdapter::new()) as Arc<dyn Adapter>);

    match cli.command {
        Command::Init { dir } => init_workspace(&dir),
        Command::Render {
            template,
            seed,
            format,
            out,
            params,
        } => render_cmd(
            &registry,
            &template,
            &seed,
            format.into(),
            format.into(),
            &out,
            &params,
        ),
        Command::Share {
            template,
            seed,
            out,
            params,
            host,
        } => share_cmd(&registry, &template, &seed, &out, &params, &host),
        Command::List => list_cmd(),
        Command::Random => random_cmd(),
        Command::Verify {
            template,
            seed,
            expected_hash,
        } => verify_cmd(&registry, &template, &seed, expected_hash.as_deref()),
        Command::Doctor => doctor_cmd(&registry),
        Command::Url {
            template,
            seed,
            host,
        } => {
            println!(
                "{}/p/{}/{}",
                host.trim_end_matches('/'),
                template,
                encode_seed(&seed)
            );
            Ok(())
        }
        Command::Serve { addr, root } => serve_cmd(addr, root),
    }
}

/// Run the self-hosted gallery server. Blocks until the process is killed.
fn serve_cmd(addr: String, root: PathBuf) -> Result<()> {
    let socket: std::net::SocketAddr = addr
        .parse()
        .with_context(|| format!("invalid --addr {addr:?}"))?;
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()?.join(root)
    };
    let state = std::sync::Arc::new(seed_canvas_server::ServerState::new(&root));
    let state_for_runtime = state.clone();
    let rt = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    eprintln!(
        "🌱 seed-canvas server starting at http://{addr}/ (workspace: {})",
        root.display()
    );
    rt.block_on(async move { seed_canvas_server::serve(socket, state_for_runtime).await })
        .context("server error")?;
    Ok(())
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

fn build_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(Arc::new(ServerAdapter::new()) as Arc<dyn Adapter>);
    registry.register(Arc::new(SvgAdapter::new()) as Arc<dyn Adapter>);
    registry
}

fn resolve_template(id: &str) -> Result<seed_canvas_core::template::Template> {
    match id {
        "galaxy" => Ok(galaxy::build()),
        other => Err(anyhow!(
            "unknown template {:?}; run `seed-canvas list` to see installed templates",
            other
        )),
    }
}

fn parse_params(raw: &str) -> Result<serde_json::Value> {
    serde_json::from_str(raw).with_context(|| format!("--params must be valid JSON, got {raw:?}"))
}

fn render_cmd(
    registry: &AdapterRegistry,
    template_id: &str,
    seed_str: &str,
    adapter: AdapterKind,
    format: OutputFormat,
    out: &Path,
    params_json: &str,
) -> Result<()> {
    let template = resolve_template(template_id)?;
    let params = parse_params(params_json)?;
    let request = RenderRequest {
        seed: Seed::from_string(seed_str),
        params,
        adapter,
        format,
    };
    let output = render(&template, &request, registry)?;
    write_atomically(out, &output.bytes)?;
    eprintln!(
        "✓ wrote {} bytes ({:?}) to {} (content_hash={}…)",
        output.bytes.len(),
        output.format,
        out.display(),
        &output.content_hash[..16.min(output.content_hash.len())]
    );
    Ok(())
}

fn share_cmd(
    registry: &AdapterRegistry,
    template_id: &str,
    seed_str: &str,
    out: &Path,
    params_json: &str,
    host: &str,
) -> Result<()> {
    render_cmd(
        registry,
        template_id,
        seed_str,
        AdapterKind::Server,
        OutputFormat::Png,
        out,
        params_json,
    )?;
    let seed = Seed::from_string(seed_str);
    let url = format!(
        "{}/p/{}/{}",
        host.trim_end_matches('/'),
        template_id,
        seed.handle()
    );
    println!("{url}");
    Ok(())
}

fn list_cmd() -> Result<()> {
    let registry = build_registry();
    println!("Installed templates:");
    let template_id = "galaxy";
    let template = resolve_template(template_id)?;
    let m = template.manifest();
    println!(
        "  • {id} ({name}, v{version}) — {desc}",
        id = m.id,
        name = m.name,
        version = m.version,
        desc = m.description,
    );
    println!("\nRegistered adapters:");
    for kind in registry.kinds() {
        println!("  • {kind:?}");
    }
    Ok(())
}

fn random_cmd() -> Result<()> {
    let seed = Seed::random();
    println!("{}", seed.raw());
    Ok(())
}

fn verify_cmd(
    registry: &AdapterRegistry,
    template_id: &str,
    seed_str: &str,
    expected_hash_file: Option<&Path>,
) -> Result<()> {
    let template = resolve_template(template_id)?;
    let params = serde_json::json!({});
    let request = RenderRequest {
        seed: Seed::from_string(seed_str),
        params,
        adapter: AdapterKind::Server,
        format: OutputFormat::Json,
    };
    let output = render(&template, &request, registry)?;

    let expected = expected_hash_file
        .map(std::fs::read_to_string)
        .transpose()
        .context("failed to read --expected-hash file")?
        .map(|s| s.trim().to_owned());

    if let Some(expected) = expected {
        if expected != output.content_hash {
            bail!(
                "drift detected: got {} but expected {}",
                output.content_hash,
                expected
            );
        }
        println!("✓ hash matches ({} bytes)", output.bytes.len());
    } else {
        println!("{}", output.content_hash);
    }
    Ok(())
}

fn doctor_cmd(registry: &AdapterRegistry) -> Result<()> {
    println!("seed-canvas doctor\n");

    println!("• toolchain");
    println!(
        "    rustc   {}",
        rustc_version_runtime().unwrap_or_else(|| "unknown".into())
    );
    println!(
        "    profile {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );

    println!("\n• adapters");
    for kind in AdapterKind::all() {
        let status = if registry.kinds().contains(kind) {
            "registered"
        } else {
            "not registered"
        };
        println!("    {kind:?}  {status}");
    }

    println!("\n• built-in templates");
    let id = "galaxy";
    let template = resolve_template(id)?;
    println!(
        "    {id}  v{version} ({width}x{height})",
        version = template.manifest().version,
        width = template.manifest().canvas.width,
        height = template.manifest().canvas.height,
    );

    Ok(())
}

fn init_workspace(dir: &Path) -> Result<()> {
    if dir.exists() {
        bail!("directory {:?} already exists; refusing to overwrite", dir);
    }
    std::fs::create_dir_all(dir)?;
    let seed_canvas_toml = dir.join("seed-canvas.toml");
    std::fs::write(
        &seed_canvas_toml,
        "# seed-canvas workspace\n\
         [workspace]\n\
         name = \"my-gallery\"\n\
         templates = []\n\
         default_template = \"galaxy\"\n\
         default_adapter = \"server\"\n",
    )?;
    std::fs::create_dir_all(dir.join("templates"))?;
    std::fs::create_dir_all(dir.join("artworks"))?;
    println!("✓ initialized workspace at {}", dir.display());
    Ok(())
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("sc")
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn encode_seed(seed: &str) -> String {
    Seed::from_string(seed).handle()
}

fn rustc_version_runtime() -> Option<String> {
    // Compile-time rustc version; the environment variable is set by cargo.
    option_env!("CARGO_PKG_RUST_VERSION").map(str::to_owned)
}
