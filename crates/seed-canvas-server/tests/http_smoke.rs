//! HTTP integration tests: boot the real router on an ephemeral port
//! and exercise every route with plain `ureq` requests.

use std::net::SocketAddr;
use std::sync::Arc;

use seed_canvas_server::{ServerError, ServerState};

async fn spawn_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    // Each test gets its own workspace so parallel tests never contend
    // on the same SQLite file.
    let root = tempfile::tempdir().expect("tempdir").keep();
    let state = Arc::new(ServerState::new(root));
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = seed_canvas_server::routes::router(state);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (bound, handle)
}

#[tokio::test(flavor = "multi_thread")]
async fn gallery_routes_smoke() -> Result<(), ServerError> {
    let (addr, handle) = spawn_server().await;
    let base = format!("http://{addr}");

    let (status, body) = get(&format!("{base}/"));
    assert_eq!(status, 200, "GET / -> {status}");
    assert!(body.contains("Seed is the Artwork"), "index body");

    let (status, _) = get(&format!("{base}/about"));
    assert_eq!(status, 200, "GET /about");

    let (status, body) = get(&format!("{base}/t/galaxy"));
    assert_eq!(status, 200, "GET /t/galaxy");
    assert!(body.contains("Galaxy"), "template page body");

    let (status, body) = get(&format!("{base}/p/galaxy/cosmos"));
    assert_eq!(status, 200, "GET /p/galaxy/cosmos");
    assert!(body.contains("og:image"), "artwork page must emit OG tags");

    let (status, body) = get(&format!("{base}/embed/galaxy/cosmos"));
    assert_eq!(status, 200, "GET /embed");
    assert!(body.contains("<img"), "embed must embed an image");

    let (status, _) = get(&format!("{base}/p/nope/cosmos"));
    assert_eq!(status, 404, "unknown template must 404");

    handle.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn artwork_bytes_are_deterministic_and_typed() -> Result<(), ServerError> {
    let (addr, handle) = spawn_server().await;
    let base = format!("http://{addr}");

    let first = get_bytes(&format!("{base}/art/galaxy/cosmos.png"));
    let second = get_bytes(&format!("{base}/art/galaxy/cosmos.png"));
    assert_eq!(first.0, 200);
    assert_eq!(first.1, second.1, "PNG bytes must be identical");

    let svg = get_bytes(&format!("{base}/art/galaxy/cosmos.svg"));
    assert_eq!(svg.0, 200);
    assert!(svg.1.starts_with(b"<?xml"), "SVG must be well-formed");

    let missing = get(&format!("{base}/art/galaxy/cosmos.bmp"));
    assert_eq!(missing.0, 500, "unknown extension is a server-side 500");

    handle.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn og_image_is_1200x630() -> Result<(), ServerError> {
    let (addr, handle) = spawn_server().await;
    let base = format!("http://{addr}");

    let (status, bytes) = get_bytes(&format!("{base}/og/galaxy/cosmos"));
    assert_eq!(status, 200);
    assert!(bytes.len() > 10_000, "OG PNG must have real content");
    // PNG IHDR: width/height are big-endian u32 at fixed offsets.
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    assert_eq!((w, h), (1200, 630), "OG image must be 1200x630");

    handle.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn api_roundtrip() -> Result<(), ServerError> {
    let (addr, handle) = spawn_server().await;
    let base = format!("http://{addr}");

    // POST /api/render
    let agent = ureq::AgentBuilder::new().build();
    let response = agent
        .post(&format!("{base}/api/render"))
        .send_json(serde_json::json!({
            "template": "galaxy",
            "seed": "it-test",
            "format": "png",
            "params": {"count": 100}
        }));
    let response = match response {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            panic!(
                "POST /api/render -> {code}: {}",
                r.into_string().unwrap_or_default()
            );
        }
        Err(e) => panic!("POST /api/render failed: {e}"),
    };
    assert_eq!(response.status(), 200);
    let payload: serde_json::Value = response
        .into_json()
        .map_err(|e| ServerError::Render(e.to_string()))?;
    assert_eq!(payload["template"], "galaxy");
    assert!(payload["content_hash"].as_str().unwrap().len() == 64);

    // GET /api/artworks must include it now.
    let (status, body) = get(&format!("{base}/api/artworks"));
    assert_eq!(status, 200);
    assert!(body.contains("it-test"), "rendered artwork must be stored");

    // GET /api/search
    let (status, body) = get(&format!("{base}/api/search?q=it-test"));
    assert_eq!(status, 200);
    assert!(body.contains("it-test"), "FTS must find the artwork");

    handle.abort();
    Ok(())
}

// --- helpers -------------------------------------------------------------

fn get(url: &str) -> (u16, String) {
    let agent = ureq::AgentBuilder::new().build();
    match agent.get(url).call() {
        Ok(resp) => {
            let status = resp.status();
            (status, resp.into_string().unwrap_or_default())
        }
        Err(ureq::Error::Status(code, resp)) => (code, resp.into_string().unwrap_or_default()),
        Err(e) => panic!("GET {url} failed: {e}"),
    }
}

fn get_bytes(url: &str) -> (u16, Vec<u8>) {
    let agent = ureq::AgentBuilder::new().build();
    match agent.get(url).call() {
        Ok(resp) => {
            let status = resp.status();
            let mut bytes = Vec::new();
            use std::io::Read as _;
            resp.into_reader().read_to_end(&mut bytes).unwrap();
            (status, bytes)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let mut bytes = Vec::new();
            let _ = resp.into_reader().read_to_end(&mut bytes);
            (code, bytes)
        }
        Err(e) => panic!("GET {url} failed: {e}"),
    }
}
