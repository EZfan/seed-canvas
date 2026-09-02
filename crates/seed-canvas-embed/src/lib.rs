//! Self-contained HTML export for seed-canvas.
//!
//! Every function in this crate produces a **fully self-contained**
//! document: artwork bytes are embedded as `data:` URLs, so the output
//! renders identically offline, in email, or from a USB stick. No CDN,
//! no fonts to fetch, no JavaScript required.
//!
//! * [`artwork_page`] — one artwork, centered, with its seed and a
//!   footer crediting the template.
//! * [`gallery_page`] — a responsive grid of artworks with per-item
//!   captions.
//! * [`iframe_snippet`] — a copy-pasteable `<iframe>` tag pointing at a
//!   running gallery server.
//! * [`og_tags`] — Open Graph / Twitter meta tags for link previews.

#![deny(missing_docs)]

use base64::Engine as _;
use thiserror::Error;

/// Errors raised while producing embeddable documents.
#[derive(Debug, Error)]
pub enum EmbedError {
    /// The artwork bytes were empty, which would produce a broken image.
    #[error("artwork bytes are empty")]
    EmptyBytes,
}

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Encode bytes as a `data:` URL with the given MIME type.
#[must_use]
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

/// Escape characters that are special in HTML text nodes and attributes.
#[must_use]
pub fn html_escape(s: &str) -> String {
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

/// One row in [`gallery_page`].
#[derive(Clone, Debug)]
pub struct GalleryItem<'a> {
    /// Template identifier, e.g. `"galaxy"`.
    pub template_id: &'a str,
    /// Raw seed string.
    pub seed: &'a str,
    /// Encoded artwork bytes (PNG or SVG).
    pub bytes: &'a [u8],
    /// MIME type of `bytes` — `"image/png"` or `"image/svg+xml"`.
    pub mime: &'a str,
}

/// Build a self-contained HTML page for a single artwork.
///
/// # Errors
///
/// Returns [`EmbedError::EmptyBytes`] when `bytes` is empty.
pub fn artwork_page(
    template_id: &str,
    template_name: &str,
    seed: &str,
    bytes: &[u8],
    mime: &str,
) -> Result<String, EmbedError> {
    if bytes.is_empty() {
        return Err(EmbedError::EmptyBytes);
    }
    let img = data_url(mime, bytes);
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{tid}/{seed} · seed-canvas</title>
<style>
:root {{ color-scheme: dark; }}
html, body {{ margin: 0; padding: 0; height: 100%; background: #0a0a0a; }}
body {{ display: grid; place-items: center; font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }}
figure {{ margin: 0; text-align: center; max-width: 96vmin; }}
img {{ max-width: 92vmin; max-height: 86vh; display: block; margin: 0 auto 12px; border-radius: 6px; }}
figcaption {{ color: #999; }}
figcaption .seed {{ color: #76e4f7; }}
figcaption a {{ color: #999; text-decoration: none; border-bottom: 1px dotted #555; }}
</style>
</head>
<body>
<figure>
<img src="{img}" alt="{tid} / {seed}">
<figcaption>
<span class="seed">{seed}</span> · {tname} ·
<a href="https://github.com/EZfan/seed-canvas">seed-canvas</a>
</figcaption>
</figure>
</body>
</html>
"#,
        tid = html_escape(template_id),
        seed = html_escape(seed),
        tname = html_escape(template_name),
        img = img,
    ))
}

/// Build a self-contained gallery page: a responsive grid of artworks
/// with per-item captions.
///
/// # Errors
///
/// Returns [`EmbedError::EmptyBytes`] when any item has empty bytes.
pub fn gallery_page(title: &str, items: &[GalleryItem<'_>]) -> Result<String, EmbedError> {
    let mut cards = String::new();
    for item in items {
        if item.bytes.is_empty() {
            return Err(EmbedError::EmptyBytes);
        }
        cards.push_str(&format!(
            r#"<figure>
<a href="{img}"><img loading="lazy" src="{img}" alt="{tid} / {seed}"></a>
<figcaption><span class="seed">{seed}</span> · {tid}</figcaption>
</figure>
"#,
            img = data_url(item.mime, item.bytes),
            tid = html_escape(item.template_id),
            seed = html_escape(item.seed),
        ));
    }
    let generated = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · seed-canvas</title>
<style>
:root {{ color-scheme: dark; }}
html, body {{ margin: 0; padding: 0; background: #0a0a0a; color: #ededed;
  font: 15px/1.5 ui-sans-serif, system-ui, sans-serif; }}
header {{ padding: 40px 24px 8px; text-align: center; }}
h1 {{ margin: 0 0 4px; font-size: 28px; letter-spacing: -0.01em; }}
header p {{ margin: 0; color: #999; font-size: 14px; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 18px; padding: 24px; max-width: 1400px; margin: 0 auto; }}
figure {{ margin: 0; background: #141414; border: 1px solid #262626;
  border-radius: 12px; overflow: hidden; }}
img {{ width: 100%; aspect-ratio: 1/1; object-fit: cover; display: block; }}
figcaption {{ padding: 8px 12px; font: 12px/1 ui-monospace, monospace; color: #999; }}
figcaption .seed {{ color: #76e4f7; }}
footer {{ text-align: center; padding: 24px; color: #777;
  font: 12px/1.5 ui-monospace, monospace; }}
footer a {{ color: #999; }}
</style>
</head>
<body>
<header>
<h1>{title}</h1>
<p>{n} artworks · deterministic · rendered by seed-canvas</p>
</header>
<div class="grid">
{cards}</div>
<footer>Generated {generated} · <a href="https://github.com/EZfan/seed-canvas">seed-canvas</a> — Seed is the Artwork.</footer>
</body>
</html>
"#,
        title = html_escape(title),
        n = items.len(),
        cards = cards,
        generated = generated,
    ))
}

/// Build an `<iframe>` snippet pointing at a running gallery server.
///
/// The width/height default to a square matching the canvas; pass
/// smaller values for card-style embeds.
#[must_use]
pub fn iframe_snippet(
    host: &str,
    template_id: &str,
    seed: &str,
    width: u32,
    height: u32,
) -> String {
    format!(
        r#"<iframe src="{host}/embed/{tid}/{seed}" width="{w}" height="{h}" frameborder="0" loading="lazy" title="seed-canvas: {tid}/{seed}"></iframe>"#,
        host = host.trim_end_matches('/'),
        tid = html_escape(template_id),
        seed = html_escape(seed),
        w = width,
        h = height,
    )
}

/// Open Graph + Twitter meta tags for a single artwork page.
///
/// `og_image_url` should point at the server's `/og/:template/:seed`
/// route (1200×630 PNG) when the gallery is served over HTTP; for
/// static hosting, pass a `data:` URL produced by [`data_url`].
#[must_use]
pub fn og_tags(title: &str, description: &str, og_image_url: &str, page_url: &str) -> String {
    format!(
        r#"<meta property="og:type" content="website">
<meta property="og:title" content="{title}">
<meta property="og:description" content="{desc}">
<meta property="og:image" content="{img}">
<meta property="og:url" content="{url}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{title}">
<meta name="twitter:description" content="{desc}">
<meta name="twitter:image" content="{img}">"#,
        title = html_escape(title),
        desc = html_escape(description),
        img = html_escape(og_image_url),
        url = html_escape(page_url),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1PX: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // fake but non-empty
    ];

    #[test]
    fn data_url_encodes_bytes() {
        let url = data_url("image/png", b"hi");
        assert!(url.starts_with("data:image/png;base64,"));
        // "hi" -> "aGk="
        assert!(url.ends_with("aGk="));
    }

    #[test]
    fn artwork_page_is_self_contained() {
        let page = artwork_page("galaxy", "Galaxy", "cosmos", PNG_1PX, "image/png").unwrap();
        assert!(page.contains("data:image/png;base64,"));
        assert!(page.contains("cosmos"));
        assert!(!page.contains("src=\"http")); // no external references
    }

    #[test]
    fn artwork_page_rejects_empty_bytes() {
        assert!(artwork_page("galaxy", "Galaxy", "cosmos", &[], "image/png").is_err());
    }

    #[test]
    fn gallery_page_contains_all_items() {
        let items = [
            GalleryItem {
                template_id: "galaxy",
                seed: "cosmos",
                bytes: PNG_1PX,
                mime: "image/png",
            },
            GalleryItem {
                template_id: "particles",
                seed: "aurora",
                bytes: PNG_1PX,
                mime: "image/png",
            },
        ];
        let page = gallery_page("My Gallery", &items).unwrap();
        assert!(page.contains("cosmos"));
        assert!(page.contains("aurora"));
        assert_eq!(page.matches("data:image/png;base64,").count(), 4); // 2 imgs × (src + href)
    }

    #[test]
    fn iframe_snippet_points_at_embed_route() {
        let s = iframe_snippet("https://art.example.com/", "galaxy", "cosmos", 512, 512);
        assert!(s.contains(r#"src="https://art.example.com/embed/galaxy/cosmos""#));
        assert!(s.contains(r#"width="512""#));
    }

    #[test]
    fn og_tags_escape_quotes() {
        let tags = og_tags(
            r#"Galaxy "cosmos""#,
            "desc & more",
            "https://x/og.png",
            "https://x/p/galaxy/cosmos",
        );
        assert!(tags.contains("Galaxy &quot;cosmos&quot;"));
        assert!(tags.contains("desc &amp; more"));
    }

    #[test]
    fn html_escape_round_trip() {
        assert_eq!(html_escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#x27;");
    }
}
