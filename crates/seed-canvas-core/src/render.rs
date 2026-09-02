//! Top-level render pipeline.
//!
//! The [`render`] function is the only entry point adapters and CLI need
//! to know about. It encapsulates the full pipeline:
//!
//! 1. Validate `params` against the template's JSON Schema.
//! 2. Construct a [`Surface`] from the requested adapter.
//! 3. Run the template's entry function.
//! 4. Encode the surface into the requested format.
//! 5. Hash the output bytes for content addressing.

use crate::adapter::{AdapterKind, AdapterRegistry};
use crate::seed::Seed;
use crate::surface::{OutputFormat, SurfaceError};
use crate::template::{Template, TemplateError};
use thiserror::Error;

/// A single render request — everything needed to produce one artwork.
#[derive(Clone, Debug)]
pub struct RenderRequest {
    /// Deterministic seed stream.
    pub seed: Seed,
    /// Raw parameters, validated against the template's schema before
    /// rendering.
    pub params: serde_json::Value,
    /// Desired adapter.
    pub adapter: AdapterKind,
    /// Desired output format. PNG for raster adapters, SVG for the SVG
    /// adapter, JSON for debugging.
    pub format: OutputFormat,
}

impl RenderRequest {
    /// Construct a new request with sensible defaults.
    #[must_use]
    pub fn new(seed: Seed, adapter: AdapterKind, format: OutputFormat) -> Self {
        Self {
            seed,
            params: serde_json::Value::Object(Default::default()),
            adapter,
            format,
        }
    }

    /// Replace `params`.
    #[must_use]
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = params;
        self
    }
}

/// The result of a successful render.
#[derive(Clone, Debug)]
pub struct RenderOutput {
    /// Encoded artifact bytes (PNG / SVG / JSON).
    pub bytes: Vec<u8>,
    /// Hex-encoded SHA-256 of `bytes`. Used as a content address so the
    /// gallery can deduplicate and the `verify` command can detect drift.
    pub content_hash: String,
    /// Adapter that produced the output.
    pub adapter: AdapterKind,
    /// Format of the output bytes.
    pub format: OutputFormat,
}

/// Errors raised by [`render`].
#[derive(Debug, Error)]
pub enum RenderError {
    /// Template manifest failed validation.
    #[error(transparent)]
    Template(#[from] TemplateError),

    /// Adapter lookup or instantiation failed.
    #[error(transparent)]
    Adapter(#[from] crate::adapter::AdapterError),

    /// Surface failed to encode its output.
    #[error(transparent)]
    Surface(#[from] SurfaceError),

    /// Hashing the output failed (extremely unlikely — would indicate a
    /// SHA-256 implementation bug).
    #[error("failed to hash output: {0}")]
    Hash(String),
}

/// Run the full render pipeline.
///
/// # Errors
///
/// Any of [`TemplateError`], [`AdapterError`], or [`SurfaceError`] may
/// surface; they are wrapped in [`RenderError`] for ergonomic callers.
pub fn render(
    template: &Template,
    request: &RenderRequest,
    registry: &AdapterRegistry,
) -> Result<RenderOutput, RenderError> {
    // 1. Validate params.
    let params = template.validate_params(request.params.clone())?;

    // 2. Construct surface.
    let mut surface = registry.create_surface(request.adapter, request)?;

    // 3. Run the template.
    let mut seed = request.seed.clone();
    template.render(&mut seed, &params, surface.as_mut())?;

    // 4. Encode.
    let bytes = surface.encode(request.format)?;

    // 5. Hash.
    let content_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };

    Ok(RenderOutput {
        bytes,
        content_hash,
        adapter: request.adapter,
        format: request.format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{Adapter, BoxedAdapter};
    use crate::surface::{Color, Surface, Vec2};
    use crate::template::{Author, CanvasSize, TemplateManifest};
    use std::sync::Arc;

    struct TrivialAdapter;
    impl Adapter for TrivialAdapter {
        fn kind(&self) -> AdapterKind {
            AdapterKind::Server
        }

        fn create_surface(
            &self,
            request: &RenderRequest,
        ) -> Result<Box<dyn Surface>, crate::adapter::AdapterError> {
            Ok(Box::new(TrivialSurface {
                seed_raw: request.seed.raw().to_owned(),
            }))
        }
    }

    struct TrivialSurface {
        seed_raw: String,
    }
    impl Surface for TrivialSurface {
        fn clear(&mut self, _color: Color) {}
        fn fill_circle(&mut self, _c: Vec2, _r: f64, _color: Color) {}
        fn stroke_line(&mut self, _a: Vec2, _b: Vec2, _w: f64, _color: Color) {}
        fn fill_rect(&mut self, _p: Vec2, _s: Vec2, _color: Color) {}
        fn fill_polygon(&mut self, _pts: &[Vec2], _color: Color) {}
        fn encode(&mut self, format: OutputFormat) -> Result<Vec<u8>, SurfaceError> {
            // Bake the seed into the output so identical seeds → identical
            // bytes (test #1) and different seeds → different bytes (test #2).
            Ok(format!("seed={}-{:?}", self.seed_raw, format).into_bytes())
        }
    }

    fn dummy_template() -> Template {
        let manifest = TemplateManifest {
            id: "trivial".into(),
            name: "Trivial".into(),
            version: "0.1.0".into(),
            description: "for tests".into(),
            authors: vec![Author {
                name: "Anon".into(),
                url: None,
                email: None,
            }],
            license: "MIT".into(),
            canvas: CanvasSize {
                width: 100,
                height: 100,
            },
            tags: vec![],
            min_seed_canvas: "0.1.0".into(),
            params_schema: serde_json::json!({"type": "object"}),
            adapters: vec![AdapterKind::Server],
            thumbnail: None,
        };
        fn entry(_ctx: &mut crate::template::RenderContext<'_>) -> Result<(), TemplateError> {
            Ok(())
        }
        Template::new(manifest, entry).unwrap()
    }

    #[test]
    fn full_pipeline_produces_deterministic_bytes() {
        let template = dummy_template();
        let mut registry = AdapterRegistry::new();
        registry.register(Arc::new(TrivialAdapter) as BoxedAdapter);

        let req = RenderRequest {
            seed: Seed::from_string("cosmos"),
            params: serde_json::json!({}),
            adapter: AdapterKind::Server,
            format: OutputFormat::Json,
        };

        let a = render(&template, &req, &registry).unwrap();
        let b = render(&template, &req, &registry).unwrap();
        assert_eq!(a.bytes, b.bytes);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn different_seeds_produce_different_output() {
        let template = dummy_template();
        let mut registry = AdapterRegistry::new();
        registry.register(Arc::new(TrivialAdapter) as BoxedAdapter);

        let req_a = RenderRequest {
            seed: Seed::from_string("a"),
            params: serde_json::json!({}),
            adapter: AdapterKind::Server,
            format: OutputFormat::Json,
        };
        let req_b = RenderRequest {
            seed: Seed::from_string("b"),
            params: serde_json::json!({}),
            adapter: AdapterKind::Server,
            format: OutputFormat::Json,
        };

        let a = render(&template, &req_a, &registry).unwrap();
        let b = render(&template, &req_b, &registry).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }
}
