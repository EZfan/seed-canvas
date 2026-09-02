//! Self-hosted HTTP gallery server for seed-canvas.
//!
//! Serves three classes of resources:
//!
//! * **Gallery HTML** — `/`, `/p/:template/:seed`, `/t/:template`, `/about`,
//!   generated server-side with the [`templates`] module.
//! * **Static assets** — CSS and JS embedded in the binary via
//!   [`axum::response::IntoResponse`] handlers.
//! * **JSON API** — `/api/render`, `/api/artworks`, `/api/search` for
//!   clients that prefer HTTP over the Rust SDK.
//!
//! The server is intentionally tiny: a single binary, no JavaScript
//! build step, no client-side framework. Templates render with vanilla
//! CSS that respects `prefers-color-scheme`.

#![deny(missing_docs)]

pub mod handlers;
pub mod routes;
pub mod state;
pub mod templates;

pub use state::ServerState;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// Errors raised by the server crate.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Failed to bind the TCP listener.
    #[error("bind {addr}: {source}")]
    Bind {
        /// Address we tried to bind.
        addr: SocketAddr,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },

    /// std I/O error (bind failure etc).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// axum hyper error.
    #[error("hyper: {0}")]
    Hyper(#[from] hyper::Error),

    /// Storage layer error.
    #[error("storage: {0}")]
    Storage(#[from] seed_canvas_storage::StorageError),

    /// Rendering error.
    #[error("render: {0}")]
    Render(String),
}

/// Bind a TCP listener and serve forever. Blocks until SIGINT/SIGTERM.
pub async fn serve(addr: SocketAddr, state: Arc<ServerState>) -> Result<(), ServerError> {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|source| ServerError::Bind { addr, source })?;
    tracing::info!("seed-canvas server listening on http://{addr}");

    let app = routes::router(state);

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("seed-canvas server shutting down");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// Spawn the server as a tokio task. Useful for tests.
#[must_use]
pub fn spawn(addr: SocketAddr, state: Arc<ServerState>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(err) = serve(addr, state).await {
            tracing::error!("server crashed: {err}");
        }
    })
}

/// Run a server in the foreground with a maximum lifetime (for tests).
pub async fn serve_with_timeout(
    addr: SocketAddr,
    state: Arc<ServerState>,
    timeout: Duration,
) -> Result<(), ServerError> {
    let handle = spawn(addr, state);
    match tokio::time::timeout(timeout, handle).await {
        Ok(_) => Ok(()),
        Err(_) => Ok(()), // timed out — caller can stop the server by dropping state
    }
}
