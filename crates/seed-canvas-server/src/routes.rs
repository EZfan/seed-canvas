//! axum router wiring every route the server exposes.

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers;
use crate::state::ServerState;

/// Build the application router. The returned router is `Clone + Send`
/// — axum requires it so the same router can serve multiple
/// connections.
pub fn router(state: Arc<ServerState>) -> Router {
    Router::new()
        // Gallery HTML pages
        .route("/", get(handlers::index))
        .route("/about", get(handlers::about))
        .route("/t/:template", get(handlers::template_page))
        .route("/p/:template/:seed", get(handlers::artwork_page))
        // JSON API
        .route("/api/render", post(handlers::api_render))
        .route("/api/artworks", get(handlers::api_artworks))
        .route("/api/search", get(handlers::api_search))
        // Artwork bytes — `/art/:template/:seed.:ext`. axum does not allow
        // two path parameters per segment, so we capture `seed.ext` as
        // a single string and split it inside the handler.
        .route("/art/:template/:rest", get(handlers::artwork_bytes))
        // Embeddable widget
        .route("/embed/:template/:seed", get(handlers::embed_widget))
        // Open Graph share image — 1200×630 PNG via size_override.
        .route("/og/:template/:seed", get(handlers::og_image))
        // Static assets (CSS / JS)
        .route("/static/style.css", get(handlers::style_css))
        .route("/static/app.js", get(handlers::app_js))
        .with_state(state)
}
