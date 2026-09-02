//! HTTP handlers for the gallery server.
//!
//! Every page is rendered server-side via the [`crate::templates`] module
//! (no JS framework, no SSR runtime). The only client-side JS is a
//! 30-line vanilla script that handles "Copy URL" and "/" keyboard focus.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use seed_canvas_core::adapter::AdapterKind;
use seed_canvas_core::surface::OutputFormat;
use seed_canvas_core::Seed;

use crate::state::ServerState;
use crate::templates;

/// Application-wide error wrapper that maps to HTTP status codes.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The requested template does not exist.
    #[error("template not found: {0}")]
    TemplateNotFound(String),

    /// The provided seed is empty after trimming.
    #[error("invalid seed")]
    InvalidSeed,

    /// Storage layer error.
    #[error("storage: {0}")]
    Storage(#[from] seed_canvas_storage::StorageError),

    /// The adapter was asked for an unsupported format.
    #[error("format {0:?} is not supported")]
    UnsupportedFormat(OutputFormat),

    /// Render pipeline error.
    #[error("render: {0}")]
    Render(String),

    /// Join error from a background task.
    #[error("task join: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            AppError::TemplateNotFound(_) => (
                StatusCode::NOT_FOUND,
                templates::error_page(404, "Template not found", &self.to_string()),
            ),
            AppError::InvalidSeed => (
                StatusCode::BAD_REQUEST,
                templates::error_page(400, "Invalid seed", "Seed must not be empty."),
            ),
            AppError::UnsupportedFormat(_) => (
                StatusCode::NOT_IMPLEMENTED,
                templates::error_page(501, "Unsupported format", &self.to_string()),
            ),
            AppError::Storage(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                templates::error_page(500, "Storage failure", &err.to_string()),
            ),
            AppError::Render(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                templates::error_page(500, "Render failed", msg),
            ),
            AppError::Join(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                templates::error_page(500, "Task join failed", &err.to_string()),
            ),
        };
        (status, body).into_response()
    }
}

/// `GET /` — gallery index, listing most recent artworks.
pub async fn index(State(state): State<Arc<ServerState>>) -> Result<Response, AppError> {
    let artworks = state.gallery.list_artworks(48)?;
    let html = templates::index_page(&artworks, &state.list_templates());
    Ok(html.into_response())
}

/// `GET /about` — project blurb + quickstart.
pub async fn about() -> Response {
    templates::about_page().into_response()
}

/// `GET /t/:template` — template detail page.
pub async fn template_page(
    State(state): State<Arc<ServerState>>,
    Path(template_id): Path<String>,
) -> Result<Response, AppError> {
    let template = state
        .template(&template_id)
        .ok_or_else(|| AppError::TemplateNotFound(template_id.clone()))?;
    let artworks = state
        .gallery
        .list_artworks(64)?
        .into_iter()
        .filter(|a| a.template_id == template_id)
        .collect::<Vec<_>>();
    Ok(templates::template_detail_page(template.manifest(), &artworks).into_response())
}

/// `GET /p/:template/:seed` — artwork detail page.
pub async fn artwork_page(
    State(state): State<Arc<ServerState>>,
    Path((template_id, seed)): Path<(String, String)>,
) -> Result<Response, AppError> {
    if seed.trim().is_empty() || seed == "__new__" {
        return Err(AppError::InvalidSeed);
    }
    let template = state
        .template(&template_id)
        .ok_or_else(|| AppError::TemplateNotFound(template_id.clone()))?;
    let artworks = state
        .gallery
        .list_artworks(64)?
        .into_iter()
        .filter(|a| a.template_id == template_id && a.seed_raw == seed)
        .collect::<Vec<_>>();
    Ok(templates::artwork_page(template.manifest(), &seed, &artworks).into_response())
}

#[derive(Debug, Deserialize)]
/// Request body for [`api_render`].
pub struct RenderApiInput {
    /// Template identifier.
    pub template: String,
    /// Deterministic seed.
    pub seed: String,
    /// Output format (`png` or `svg`).
    #[serde(default = "default_format")]
    pub format: String,
    /// Optional JSON-encoded parameters.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

fn default_format() -> String {
    "png".to_owned()
}

fn parse_format(s: &str) -> Result<(OutputFormat, AdapterKind), AppError> {
    match s {
        "png" => Ok((OutputFormat::Png, AdapterKind::Server)),
        "svg" => Ok((OutputFormat::Svg, AdapterKind::Svg)),
        "json" => Ok((OutputFormat::Json, AdapterKind::Server)),
        other => Err(AppError::Render(format!("unknown format: {other}"))),
    }
}

/// Shared render pipeline used by every HTTP handler that produces
/// artwork bytes. Returns encoded bytes + their SHA-256.
///
/// `size` overrides the canvas when provided; templates must scale
/// their geometry from `ctx.canvas`.
fn render_artwork(
    state: &ServerState,
    template: &crate::state::BoxedTemplate,
    seed_str: &str,
    params: &serde_json::Value,
    format: OutputFormat,
    adapter: AdapterKind,
    size: Option<(u32, u32)>,
) -> Result<(Vec<u8>, String), AppError> {
    let seed = Seed::from_string(seed_str);
    let request = seed_canvas_core::render::RenderRequest {
        seed: seed.clone(),
        params: params.clone(),
        adapter,
        format,
        size_override: size,
    };
    let mut surface = state
        .registry
        .create_surface(adapter, &request)
        .map_err(|e| AppError::Render(format!("{e}")))?;
    let validated = template
        .validate(params.clone())
        .map_err(|e| AppError::Render(format!("{e}")))?;
    let canvas = size
        .map(|(w, h)| seed_canvas_core::template::CanvasSize {
            width: w,
            height: h,
        })
        .unwrap_or_else(|| template.manifest().canvas);
    let mut seed_stream = seed;
    template
        .render_into(&mut seed_stream, &validated, surface.as_mut(), canvas)
        .map_err(|e| AppError::Render(format!("{e}")))?;
    let bytes = surface
        .encode(format)
        .map_err(|e| AppError::Render(format!("{e}")))?;
    let hash = sha256_hex(&bytes);
    Ok((bytes, hash))
}

/// `POST /api/render` — render an artwork and return its content hash + URL.
pub async fn api_render(
    State(state): State<Arc<ServerState>>,
    axum::Json(input): axum::Json<RenderApiInput>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    let template = state
        .template(&input.template)
        .ok_or_else(|| AppError::TemplateNotFound(input.template.clone()))?;
    let (format, adapter) = parse_format(&input.format)?;
    let params = input
        .params
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    let (bytes, content_hash) = render_artwork(
        &state,
        &template,
        &input.seed,
        &params,
        format,
        adapter,
        None,
    )?;

    // Persist into the gallery.
    let handle = Seed::from_string(&input.seed).handle();
    let _ = state
        .gallery
        .upsert_artwork(seed_canvas_storage::NewArtwork {
            template_id: &input.template,
            template_version: &template.manifest().version,
            seed_raw: &input.seed,
            seed_handle: &handle,
            params: &params,
            format: input.format.as_str(),
            content_hash: &content_hash,
            file_path: std::path::Path::new(""),
            adapter: match adapter {
                AdapterKind::Server => "server",
                AdapterKind::Svg => "svg",
                _ => "other",
            },
            width: template.manifest().canvas.width,
            height: template.manifest().canvas.height,
        })?;

    let ext = match format {
        OutputFormat::Png => "png",
        OutputFormat::Svg => "svg",
        OutputFormat::Json => "json",
    };
    let url = format!("/art/{}/{}.{}", input.template, input.seed, ext);

    Ok(axum::Json(serde_json::json!({
        "content_hash": content_hash,
        "bytes_len": bytes.len(),
        "url": url,
        "template": input.template,
        "seed": input.seed,
        "format": ext,
        "handle": handle,
    })))
}

/// `GET /api/artworks` — list recent artworks.
pub async fn api_artworks(
    State(state): State<Arc<ServerState>>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    let rows = state.gallery.list_artworks(64)?;
    Ok(axum::Json(serde_json::json!({
        "count": rows.len(),
        "artworks": rows,
    })))
}

#[derive(Debug, Deserialize)]
/// Query string for [`api_search`].
pub struct SearchParams {
    /// FTS5 query.
    pub q: String,
    /// Maximum number of hits.
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    20
}

/// `GET /api/search?q=...` — full-text search.
pub async fn api_search(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<SearchParams>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    // Quote the whole query as an FTS5 phrase so seeds containing `-`,
    // quotes, or other operator characters are matched literally
    // instead of being parsed as query syntax.
    let quoted = format!("\"{}\"", params.q.replace('"', "\"\""));
    let rows = state.gallery.search(&quoted, params.limit)?;
    Ok(axum::Json(serde_json::json!({
        "query": params.q,
        "count": rows.len(),
        "artworks": rows,
    })))
}

/// `GET /art/:template/:seed.:ext` — serve rendered bytes directly.
pub async fn artwork_bytes(
    State(state): State<Arc<ServerState>>,
    Path((template_id, rest)): Path<(String, String)>,
) -> Result<Response, AppError> {
    // Split `seed.ext` on the last `.`.
    let (seed_str, ext) = match rest.rsplit_once('.') {
        Some((s, e)) => (s, e.to_owned()),
        None => return Err(AppError::Render(format!("missing extension in {rest:?}"))),
    };
    let template = state
        .template(&template_id)
        .ok_or_else(|| AppError::TemplateNotFound(template_id.clone()))?;
    let (format, adapter) =
        parse_format(&ext).map_err(|_| AppError::Render(format!("unknown extension: {ext}")))?;
    let params = serde_json::json!({});
    let (bytes, _) = render_artwork(&state, &template, seed_str, &params, format, adapter, None)?;
    let mime = match format {
        OutputFormat::Png => "image/png",
        OutputFormat::Svg => "image/svg+xml",
        OutputFormat::Json => "application/json",
    };
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static(mime))],
        bytes,
    )
        .into_response())
}

/// Width of the Open Graph share image in pixels.
pub const OG_WIDTH: u32 = 1200;
/// Height of the Open Graph share image in pixels.
pub const OG_HEIGHT: u32 = 630;

/// `GET /og/:template/:seed` — 1200×630 PNG for social link previews.
///
/// Rendered through the same deterministic pipeline as every other
/// artwork, just with a widescreen `size_override`, so the share image
/// is genuinely the artwork rather than a letterboxed screenshot.
pub async fn og_image(
    State(state): State<Arc<ServerState>>,
    Path((template_id, seed)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let template = state
        .template(&template_id)
        .ok_or_else(|| AppError::TemplateNotFound(template_id.clone()))?;
    let params = serde_json::json!({});
    let (bytes, _) = render_artwork(
        &state,
        &template,
        &seed,
        &params,
        OutputFormat::Png,
        AdapterKind::Server,
        Some((OG_WIDTH, OG_HEIGHT)),
    )?;
    Ok((
        [(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))],
        bytes,
    )
        .into_response())
}

/// `GET /embed/:template/:seed` — minimal HTML page suitable for an
/// `<iframe src>`.
pub async fn embed_widget(
    State(state): State<Arc<ServerState>>,
    Path((template_id, seed)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let template = state
        .template(&template_id)
        .ok_or_else(|| AppError::TemplateNotFound(template_id.clone()))?;
    Ok(templates::embed_widget(template.manifest(), &seed).into_response())
}

/// `GET /static/style.css` — bundled CSS.
pub async fn style_css() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        include_str!("../static/style.css"),
    )
        .into_response()
}

/// `GET /static/app.js` — bundled JS.
pub async fn app_js() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        include_str!("../static/app.js"),
    )
        .into_response()
}

/// Compute the hex-encoded SHA-256 of the rendered bytes.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_format_accepts_known_values() {
        assert_eq!(parse_format("png").unwrap().0, OutputFormat::Png);
        assert_eq!(parse_format("svg").unwrap().0, OutputFormat::Svg);
        assert_eq!(parse_format("json").unwrap().0, OutputFormat::Json);
    }

    #[test]
    fn parse_format_rejects_unknown() {
        assert!(parse_format("bmp").is_err());
    }

    #[test]
    fn default_format_is_png() {
        assert_eq!(default_format(), "png");
    }

    #[test]
    fn sha256_hex_is_deterministic() {
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}
