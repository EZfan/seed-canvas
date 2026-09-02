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

    /// Export a self-contained HTML page (artwork bytes embedded as
    /// data: URLs) that renders identically offline.
    Export {
        /// Template identifier (required unless --all).
        #[arg(short, long)]
        template: Option<String>,
        /// Deterministic seed (required unless --all).
        #[arg(short, long)]
        seed: Option<String>,
        /// Output HTML file path.
        #[arg(short, long, default_value = "artwork.html")]
        out: PathBuf,
        /// JSON-encoded params.
        #[arg(long, default_value = "{}")]
        params: String,
        /// Export every stored artwork in the workspace gallery as one
        /// grid page instead of a single artwork.
        #[arg(long)]
        all: bool,
        /// Title for --all gallery pages.
        #[arg(long, default_value = "My Gallery")]
        title: String,
        /// Workspace root used with --all.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },

    /// Print an `<iframe>` snippet that embeds a live artwork from a
    /// running `seed-canvas serve` instance.
    Embed {
        /// Template identifier.
        #[arg(short, long)]
        template: String,
        /// Deterministic seed.
        #[arg(short, long)]
        seed: String,
        /// Base URL of the running gallery server.
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        host: String,
        /// iframe width in CSS pixels.
        #[arg(long, default_value_t = 512)]
        width: u32,
        /// iframe height in CSS pixels.
        #[arg(long, default_value_t = 512)]
        height: u32,
    },

    /// Manage remote template registries (stored in the user config dir).
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },

    /// Look up a template in the built-in and configured registries.
    /// Built-in templates render immediately; external ones report
    /// their availability.
    Install {
        /// Template identifier to look up.
        template: String,
        /// Fetch fresh indexes from configured remote registries first.
        #[arg(long)]
        update: bool,
    },
}

/// Subcommands under `seed-canvas registry`.
#[derive(Subcommand, Debug)]
enum RegistryAction {
    /// List the built-in registry and every configured remote source.
    List,
    /// Fetch a remote index and remember its URL.
    Add {
        /// HTTPS URL of a registry index JSON document.
        url: String,
    },
    /// Forget a configured remote source.
    Remove {
        /// The URL previously added.
        url: String,
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
        Command::Export {
            template,
            seed,
            out,
            params,
            all,
            title,
            root,
        } => match (all, template, seed) {
            (true, _, _) => export_all_cmd(&title, &root, &out),
            (false, Some(template), Some(seed)) => {
                export_cmd(&registry, &template, &seed, &out, &params)
            }
            (false, _, _) => bail!("--template and --seed are required unless --all is set"),
        },
        Command::Embed {
            template,
            seed,
            host,
            width,
            height,
        } => {
            print!(
                "{}",
                seed_canvas_embed::iframe_snippet(&host, &template, &seed, width, height)
            );
            Ok(())
        }
        Command::Registry { action } => registry_action(action),
        Command::Install { template, update } => install_cmd(&template, update),
    }
}

/// Path of the user config file holding remote registry URLs.
fn registries_config_path() -> Result<PathBuf> {
    let config_dir = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(xdg) => PathBuf::from(xdg),
        None => {
            let home = std::env::var_os("HOME").context("cannot determine home directory")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(config_dir.join("seed-canvas").join("registries.json"))
}

/// Read the configured remote registry URLs.
fn load_registries() -> Result<Vec<String>> {
    let path = registries_config_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let urls: Vec<String> = serde_json::from_str(&raw).context("registries.json is malformed")?;
    Ok(urls)
}

/// Persist the configured remote registry URLs.
fn save_registries(urls: &[String]) -> Result<()> {
    let path = registries_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_vec_pretty(urls)?)?;
    Ok(())
}

fn registry_action(action: RegistryAction) -> Result<()> {
    match action {
        RegistryAction::List => {
            let builtin = seed_canvas_registry::builtin_index()?;
            println!(
                "built-in registry: {} ({} templates)",
                builtin.name,
                builtin.templates.len()
            );
            for t in &builtin.templates {
                println!(
                    "  • {id} v{ver} [{license}]{builtin}",
                    id = t.id,
                    ver = t.version,
                    license = t.license,
                    builtin = if t.builtin { " (built-in)" } else { "" },
                );
            }
            let remotes = load_registries()?;
            if remotes.is_empty() {
                println!(
                    "\nremote registries: none (add one with `seed-canvas registry add <url>`)"
                );
            } else {
                println!("\nremote registries:");
                for url in &remotes {
                    println!("  • {url}");
                }
            }
            Ok(())
        }
        RegistryAction::Add { url } => {
            // HTTPS everywhere except loopback, which developers use for
            // local registry testing.
            let is_loopback =
                url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
            if !url.starts_with("https://") && !is_loopback {
                bail!("registry URLs must use https (got {url:?})");
            }
            let cache_dir = registries_config_path()?
                .parent()
                .map(|p| p.join("cache"))
                .context("cannot resolve cache dir")?;
            let client = seed_canvas_registry::RegistryClient::new(cache_dir);
            let index = client
                .fetch_and_cache(&url)
                .context("failed to fetch registry index")?;
            let mut urls = load_registries()?;
            if urls.contains(&url) {
                println!(
                    "✓ refreshed {} ({} templates)",
                    index.name,
                    index.templates.len()
                );
            } else {
                urls.push(url.clone());
                save_registries(&urls)?;
                println!(
                    "✓ added {url} — {} ({} templates)",
                    index.name,
                    index.templates.len()
                );
            }
            Ok(())
        }
        RegistryAction::Remove { url } => {
            let mut urls = load_registries()?;
            let before = urls.len();
            urls.retain(|u| u != &url);
            if urls.len() == before {
                bail!("{url} is not in the configured registries");
            }
            save_registries(&urls)?;
            println!("✓ removed {url}");
            Ok(())
        }
    }
}

fn install_cmd(template_id: &str, update: bool) -> Result<()> {
    // 1. Built-in templates win immediately.
    if resolve_template(template_id).is_ok() {
        println!("✓ {template_id} is built into this binary — nothing to install");
        println!("  try: seed-canvas render --template {template_id} --seed hello --out out.png");
        return Ok(());
    }

    // 2. Optionally refresh remote indexes.
    let cache_dir = registries_config_path()?
        .parent()
        .map(|p| p.join("cache"))
        .context("cannot resolve cache dir")?;
    let client = seed_canvas_registry::RegistryClient::new(cache_dir);
    let remotes = load_registries()?;

    let mut found: Option<(String, seed_canvas_registry::TemplateEntry)> = None;
    for url in &remotes {
        let index = if update {
            match client.fetch_and_cache(url) {
                Ok(idx) => idx,
                Err(err) => {
                    tracing::warn!("refresh of {url} failed ({err}); using cache");
                    client.cached(url).context("no cached index for {url}")?
                }
            }
        } else {
            client
                .cached(url)
                .with_context(|| format!("{url} has no cached index; run with --update"))?
        };
        if let Some(entry) = index.get(template_id) {
            found = Some((url.clone(), entry.clone()));
            break;
        }
    }

    match found {
        Some((source, entry)) => {
            println!("found {} v{} in {}", entry.id, entry.version, source);
            println!("license: {}", entry.license);
            println!();
            println!(
                "note: external template distribution (WASM packages) is on the roadmap;"
            );
            println!(
                "      this version of seed-canvas renders the built-in templates only."
            );
            Ok(())
        }
        None => bail!(
            "template {template_id:?} not found in the built-in registry or {} configured remote(s)",
            remotes.len()
        ),
    }
}

/// Export a single artwork as a self-contained HTML page.
fn export_cmd(
    registry: &AdapterRegistry,
    template_id: &str,
    seed_str: &str,
    out: &Path,
    params_json: &str,
) -> Result<()> {
    let template = resolve_template(template_id)?;
    let params: serde_json::Value =
        serde_json::from_str(params_json).context("--params must be valid JSON")?;
    let request = RenderRequest {
        seed: Seed::from_string(seed_str),
        params,
        adapter: AdapterKind::Server,
        format: OutputFormat::Png,
        size_override: None,
    };
    let output = render(&template, &request, registry)?;
    let html = seed_canvas_embed::artwork_page(
        template_id,
        &template.manifest().name,
        seed_str,
        &output.bytes,
        "image/png",
    )?;
    write_atomically(out, html.as_bytes())?;
    eprintln!(
        "✓ wrote {} ({} bytes html, artwork sha256 {}…)",
        out.display(),
        html.len(),
        &output.content_hash[..16]
    );
    Ok(())
}

/// Export every stored artwork in the workspace gallery as one grid page.
fn export_all_cmd(title: &str, root: &Path, out: &Path) -> Result<()> {
    let gallery = seed_canvas_storage::Gallery::open(root)
        .with_context(|| format!("failed to open gallery at {}", root.display()))?;
    let rows = gallery.list_artworks(i64::MAX)?;
    if rows.is_empty() {
        bail!(
            "gallery at {} has no artworks; render some first with `seed-canvas render`",
            root.display()
        );
    }
    let registry = build_registry();
    // Render everything first and keep the bytes alive; GalleryItem
    // borrows from these, so both vectors must outlive `html`.
    let mut rendered = Vec::with_capacity(rows.len());
    for art in &rows {
        let template = resolve_template(&art.template_id)?;
        let request = RenderRequest {
            seed: Seed::from_string(&art.seed_raw),
            params: art.params.clone(),
            adapter: AdapterKind::Server,
            format: OutputFormat::Png,
            size_override: None,
        };
        rendered.push(render(&template, &request, &registry)?);
    }
    let items: Vec<seed_canvas_embed::GalleryItem<'_>> = rows
        .iter()
        .zip(&rendered)
        .map(|(art, output)| seed_canvas_embed::GalleryItem {
            template_id: &art.template_id,
            seed: &art.seed_raw,
            bytes: &output.bytes,
            mime: "image/png",
        })
        .collect();
    let html = seed_canvas_embed::gallery_page(title, &items)?;
    write_atomically(out, html.as_bytes())?;
    eprintln!(
        "✓ wrote {} ({} artworks, {} bytes html)",
        out.display(),
        items.len(),
        html.len()
    );
    Ok(())
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
        "particles" => Ok(particles::build()),
        "mandala" => Ok(mandala::build()),
        other => Err(anyhow!(
            "unknown template {:?}; run `seed-canvas list` to see installed templates",
            other
        )),
    }
}

/// All built-in template ids, in display order.
const BUILTIN_TEMPLATES: &[&str] = &["galaxy", "particles", "mandala"];

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
        size_override: None,
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
    for template_id in BUILTIN_TEMPLATES {
        let template = resolve_template(template_id)?;
        let m = template.manifest();
        println!(
            "  • {id} ({name}, v{version}) — {desc}",
            id = m.id,
            name = m.name,
            version = m.version,
            desc = m.description,
        );
    }
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
        size_override: None,
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
    for id in BUILTIN_TEMPLATES {
        let template = resolve_template(id)?;
        println!(
            "    {id}  v{version} ({width}x{height})",
            version = template.manifest().version,
            width = template.manifest().canvas.width,
            height = template.manifest().canvas.height,
        );
    }

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
