//! Server-side HTML rendering.
//!
//! We deliberately avoid pulling in a heavy templating engine: every
//! page is a function that returns a `String`. The strings are small
//! (~10KB per page), the gallery renders 48 artworks per page, and the
//! whole thing renders in microseconds. Adding `askama` or `handlebars`
//! would double the dependency tree without measurable benefit.

use std::fmt::Write as _;

use seed_canvas_core::Seed;
use seed_canvas_storage::Artwork;

const SHELL: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="seed-canvas">
<title>{title} · seed-canvas</title>
{head_extra}
<link rel="stylesheet" href="/static/style.css">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'><circle cx='8' cy='8' r='6' fill='%2376e4f7'/></svg>">
</head>
<body>
<header class="topbar">
  <a class="brand" href="/">🌱 seed-canvas</a>
  <nav>
    <a href="/">Gallery</a>
    <a href="/about">About</a>
    <a href="https://github.com/EZfan/seed-canvas" rel="noopener">GitHub</a>
  </nav>
</header>
<main>
{body}
</main>
<footer>
  <span>Seed is the Artwork.</span>
  <span>·</span>
  <span>Generated at {generated_at}</span>
</footer>
<script src="/static/app.js" defer></script>
</body>
</html>
"#;

fn shell(title: &str, body: &str) -> String {
    shell_with_head(title, body, "")
}

/// Like [`shell`] but injects raw HTML into `<head>` (used for OG tags).
fn shell_with_head(title: &str, body: &str, head_extra: &str) -> String {
    let now = chrono::Utc::now().to_rfc3339();
    SHELL
        .replace("{title}", &html_escape(title))
        .replace("{head_extra}", head_extra)
        .replace("{body}", body)
        .replace("{generated_at}", &now)
}

/// `GET /` — gallery index.
pub fn index_page(
    artworks: &[Artwork],
    templates: &[seed_canvas_core::template::TemplateManifest],
) -> String {
    let mut body = String::new();
    let _ = writeln!(
        body,
        r#"<section class="hero">
<h1>Seed is the Artwork.</h1>
<p class="lead">{} artworks · {} templates · deterministic by construction.</p>
<form action="/" method="get" class="seed-search">
<input name="q" type="search" placeholder="search seeds, params, formats…" autofocus>
<button>Search</button>
</form>
</section>"#,
        artworks.len(),
        templates.len(),
    );

    if artworks.is_empty() {
        let _ = writeln!(
            body,
            r#"<section class="empty">
<p>No artworks yet. Render one with:</p>
<pre><code>seed-canvas render --template galaxy --seed cosmos --out cosmos.png</code></pre>
<p>Then run <code>seed-canvas serve</code> and refresh this page.</p>
</section>"#,
        );
    } else {
        let _ = writeln!(body, r#"<section class="grid">"#);
        for art in artworks {
            let handle = Seed::from_string(&art.seed_raw).handle();
            let _ = writeln!(
                body,
                r#"<a class="card" href="/p/{tid}/{seed}">
<img loading="lazy" src="/art/{tid}/{seed}.png" alt="artwork {seed}">
<div class="meta">
<span class="tpl">{tid}</span>
<span class="seed" title="{handle}">{seed_short}</span>
<span class="format">{fmt}</span>
</div>
</a>"#,
                tid = art.template_id,
                seed = html_escape(&art.seed_raw),
                handle = handle,
                seed_short = truncate(&art.seed_raw, 16),
                fmt = art.format,
            );
        }
        let _ = writeln!(body, "</section>");
    }
    shell("Gallery", &body)
}

/// `GET /t/:template` — template detail page.
pub fn template_detail_page(
    manifest: &seed_canvas_core::template::TemplateManifest,
    artworks: &[Artwork],
) -> String {
    let mut body = String::new();
    let _ = writeln!(
        body,
        r#"<section class="template-header">
<h1>{name}</h1>
<p class="lead">{description}</p>
<dl class="meta-grid">
<dt>id</dt><dd><code>{id}</code></dd>
<dt>version</dt><dd><code>{version}</code></dd>
<dt>canvas</dt><dd>{w} × {h}</dd>
<dt>license</dt><dd>{license}</dd>
</dl>
<form action="/p/{id}/__new__" method="get" class="seed-search">
<input name="seed" type="text" placeholder="any seed string…" required>
<button>Render</button>
</form>
</section>"#,
        name = html_escape(&manifest.name),
        id = manifest.id,
        description = html_escape(&manifest.description),
        version = manifest.version,
        w = manifest.canvas.width,
        h = manifest.canvas.height,
        license = manifest.license,
    );

    if !artworks.is_empty() {
        let _ = writeln!(body, r#"<section class="grid">"#);
        for art in artworks {
            let _ = writeln!(
                body,
                r#"<a class="card" href="/p/{tid}/{seed}">
<img loading="lazy" src="/art/{tid}/{seed}.png" alt="{seed}">
<div class="meta"><span class="seed">{seed_short}</span></div>
</a>"#,
                tid = art.template_id,
                seed = html_escape(&art.seed_raw),
                seed_short = truncate(&art.seed_raw, 16),
            );
        }
        let _ = writeln!(body, "</section>");
    }
    shell(&manifest.name, &body)
}

/// `GET /p/:template/:seed` — artwork detail page.
pub fn artwork_page(
    manifest: &seed_canvas_core::template::TemplateManifest,
    seed: &str,
    artworks: &[Artwork],
) -> String {
    let mut body = String::new();
    let handle = Seed::from_string(seed).handle();
    let canonical_url = format!("/p/{}/{}", manifest.id, seed);
    let _ = writeln!(
        body,
        r#"<section class="artwork-detail">
<header>
<h1>{name} <span class="seed-mono">{seed}</span></h1>
<p class="lead"><code>/p/{tid}/{seed}</code></p>
</header>
<div class="canvas-stage">
<img src="/art/{tid}/{seed}.png" alt="{seed} rendered with {name}">
<div class="overlay-actions">
<a class="btn" href="/art/{tid}/{seed}.png" download="{tid}-{seed}.png">PNG</a>
<a class="btn" href="/art/{tid}/{seed}.svg" download="{tid}-{seed}.svg">SVG</a>
<a class="btn" href="/embed/{tid}/{seed}">Embed</a>
<button class="btn" data-copy="{canonical}">Copy URL</button>
</div>
</div>
<section class="info-grid">
<dl>
<dt>id</dt><dd><code>{tid}</code></dd>
<dt>version</dt><dd><code>{ver}</code></dd>
<dt>canvas</dt><dd>{w} × {h}</dd>
<dt>handle</dt><dd><code>{handle}</code></dd>
<dt>rendered</dt><dd>{n} times</dd>
</dl>
</section>
</section>"#,
        tid = manifest.id,
        seed = html_escape(seed),
        name = html_escape(&manifest.name),
        ver = manifest.version,
        w = manifest.canvas.width,
        h = manifest.canvas.height,
        handle = handle,
        n = artworks.len(),
        canonical = canonical_url,
    );
    // Open Graph tags so link previews (Slack, X/Twitter, iMessage, …)
    // show the artwork itself via the /og share image.
    let og_image_url = format!("/og/{}/{}", manifest.id, seed);
    let og = format!(
        r#"<meta property="og:type" content="website">
<meta property="og:title" content="{name} / {seed} — seed-canvas">
<meta property="og:description" content="Deterministic artwork {tid}/{seed}. Same seed, same image, every platform.">
<meta property="og:image" content="{img}">
<meta name="twitter:card" content="summary_large_image">"#,
        name = html_escape(&manifest.name),
        tid = manifest.id,
        seed = html_escape(seed),
        img = og_image_url,
    );
    shell_with_head(&format!("{} · {}", manifest.name, seed), &body, &og)
}

/// `GET /embed/:template/:seed` — minimal iframe-friendly page.
pub fn embed_widget(manifest: &seed_canvas_core::template::TemplateManifest, seed: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>seed-canvas embed</title>
<style>html,body{{margin:0;padding:0;height:100%;background:#0a0a0a;display:grid;place-items:center}}
img{{max-width:100%;max-height:100%;display:block}}
.caption{{position:fixed;bottom:6px;right:8px;font:11px/1.4 ui-monospace,monospace;color:#aaa;background:rgba(0,0,0,.6);padding:3px 6px;border-radius:4px}}
</style></head><body>
<img src="/art/{tid}/{seed}.png" alt="seed-canvas artwork">
<div class="caption">🌱 {name} / {seed}</div>
</body></html>"#,
        tid = manifest.id,
        seed = html_escape(seed),
        name = html_escape(&manifest.name),
    )
}

/// `GET /about` — blurb.
pub fn about_page() -> String {
    let body = r#"<section class="about">
<h1>About seed-canvas</h1>
<p>seed-canvas is an open-source, self-hostable platform for deterministic generative art.</p>
<p>The same seed + template always produces the same artwork — byte-identical on every
platform, in every format. <code>/p/galaxy/sc_1eebd7175c6b0b26921647f4</code> is a
permanent, content-addressed identifier.</p>
<h2>Quickstart</h2>
<pre><code>cargo install seed-canvas-cli
seed-canvas render --template galaxy --seed cosmos --out cosmos.png
seed-canvas serve                # open http://localhost:8080</code></pre>
<h2>Five principles</h2>
<ol>
<li>Determinism over randomness.</li>
<li>Local-first, self-hosted, no telemetry.</li>
<li>Templates are pure functions.</li>
<li>Bytes are portable across formats.</li>
<li>The URL is the artwork.</li>
</ol>
</section>"#;
    shell("About", body)
}

/// Error page used by [`AppError::into_response`].
pub fn error_page(code: u16, title: &str, body: &str) -> String {
    let html = format!(
        r#"<section class="error">
<h1>{code}</h1>
<h2>{title}</h2>
<p>{body}</p>
<p><a href="/">← back to gallery</a></p>
</section>"#,
        code = code,
        title = html_escape(title),
        body = html_escape(body),
    );
    shell(&format!("Error {code}"), &html)
}

/// Escape characters that have special meaning in HTML.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Truncate a string to `n` characters with an ellipsis if needed.
fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_handles_specials() {
        assert_eq!(html_escape("a<b>c&d"), "a&lt;b&gt;c&amp;d");
        assert_eq!(html_escape("\"x\""), "&quot;x&quot;");
    }

    #[test]
    fn truncate_handles_unicode() {
        assert_eq!(truncate("cosmos", 10), "cosmos");
        assert_eq!(truncate("cosmos-long", 6), "cosmos…");
    }

    #[test]
    fn index_page_renders_with_no_artworks() {
        let s = index_page(&[], &[]);
        assert!(s.contains("Seed is the Artwork."));
        assert!(s.contains("No artworks yet"));
    }
}
