//! The [`Adapter`] trait and the registry that resolves an [`AdapterKind`]
//! to a concrete backend implementation.
//!
//! Adapters live in separate crates (`seed-canvas-adapter-server`,
//! `seed-canvas-adapter-svg`, etc.). This module only defines the
//! dispatch surface so the core crate stays graphics-backend-free.

use crate::render::RenderRequest;
use crate::surface::{OutputFormat, Surface};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// All render backends seed-canvas knows about.
///
/// Adding a new backend is a three-step change:
///
/// 1. Add a variant here.
/// 2. Implement [`Adapter::kind`] in the new adapter crate.
/// 3. Register the adapter at startup (`AdapterRegistry::register`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    /// CPU rasterizer producing 8-bit-per-channel PNG.
    Server,
    /// Declarative SVG output (no rasterization).
    Svg,
    /// Browser-side 2D canvas. Used by the web viewer.
    Canvas2d,
    /// Browser-side WebGL 2 (future).
    Webgl,
    /// Browser-side WebGPU (future).
    Webgpu,
}

impl AdapterKind {
    /// Lowercase string identifier used in CLI flags and manifests.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Svg => "svg",
            Self::Canvas2d => "canvas2d",
            Self::Webgl => "webgl",
            Self::Webgpu => "webgpu",
        }
    }

    /// Parse from a string. Case-insensitive.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::UnknownKind`] for unknown values.
    pub fn parse(s: &str) -> Result<Self, AdapterError> {
        match s.to_lowercase().as_str() {
            "server" => Ok(Self::Server),
            "svg" => Ok(Self::Svg),
            "canvas2d" => Ok(Self::Canvas2d),
            "webgl" => Ok(Self::Webgl),
            "webgpu" => Ok(Self::Webgpu),
            other => Err(AdapterError::UnknownKind(other.to_owned())),
        }
    }

    /// All variants. Used by the `seed-canvas doctor` CLI command.
    #[must_use]
    pub const fn all() -> &'static [AdapterKind] {
        &[
            Self::Server,
            Self::Svg,
            Self::Canvas2d,
            Self::Webgl,
            Self::Webgpu,
        ]
    }
}

/// Errors raised by the adapter layer.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// User requested an adapter that is not registered.
    #[error("no adapter registered for {0:?} (run `seed-canvas doctor`)")]
    Unregistered(AdapterKind),

    /// User passed a string the parser could not recognize.
    #[error("unknown adapter kind: {0:?} (valid: server, svg, canvas2d, webgl, webgpu)")]
    UnknownKind(String),

    /// Adapter could not produce the requested format.
    #[error("adapter {0:?} does not support format {1:?}")]
    UnsupportedFormat(AdapterKind, OutputFormat),

    /// Adapter-specific rendering failure.
    #[error("adapter {0:?} render failed: {1}")]
    Render(AdapterKind, String),
}

/// Trait every backend adapter implements.
///
/// Adapters are stateless factories: calling [`Adapter::create_surface`]
/// returns a fresh [`Surface`] ready to be drawn on. Adapters may cache
/// state internally (e.g. font loaders) but must be `Send + Sync` so they
/// can be shared across CLI invocations and HTTP requests.
pub trait Adapter: Send + Sync {
    /// Which kind of adapter this is.
    fn kind(&self) -> AdapterKind;

    /// Construct a new surface sized to `request`.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::UnsupportedFormat`] if the adapter cannot
    /// honor the requested output format, or any adapter-specific error.
    fn create_surface(&self, request: &RenderRequest) -> Result<Box<dyn Surface>, AdapterError>;

    /// Quick check: can this adapter produce `format`? Default implementation
    /// returns `true`; adapters with format restrictions override this.
    fn supports_format(&self, format: OutputFormat) -> bool {
        let _ = format;
        true
    }
}

/// Type-erased adapter reference.
pub type BoxedAdapter = Arc<dyn Adapter>;

/// Registry that resolves [`AdapterKind`] to a concrete backend.
///
/// The registry is built once at startup by `main()` and passed by
/// reference into the rest of the system.
#[derive(Default, Clone)]
pub struct AdapterRegistry {
    by_kind: HashMap<AdapterKind, BoxedAdapter>,
}

impl AdapterRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an adapter. Overwrites any previous registration for the
    /// same kind.
    pub fn register(&mut self, adapter: BoxedAdapter) {
        self.by_kind.insert(adapter.kind(), adapter);
    }

    /// Resolve an adapter by kind.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Unregistered`] when no adapter is
    /// registered for `kind`.
    pub fn get(&self, kind: AdapterKind) -> Result<&dyn Adapter, AdapterError> {
        self.by_kind
            .get(&kind)
            .map(AsRef::as_ref)
            .ok_or(AdapterError::Unregistered(kind))
    }

    /// List all registered kinds. Used by the gallery's per-template
    /// dropdown.
    #[must_use]
    pub fn kinds(&self) -> Vec<AdapterKind> {
        let mut kinds: Vec<_> = self.by_kind.keys().copied().collect();
        kinds.sort_by_key(|k| k.as_str());
        kinds
    }

    /// Construct a surface for `request`.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Unregistered`] or any error from
    /// [`Adapter::create_surface`].
    pub fn create_surface(
        &self,
        kind: AdapterKind,
        request: &RenderRequest,
    ) -> Result<Box<dyn Surface>, AdapterError> {
        let adapter = self.get(kind)?;
        adapter.create_surface(request)
    }
}

impl std::fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("registered", &self.kinds())
            .finish()
    }
}

// Kept for backwards compatibility with earlier internal imports.
pub use crate::surface::SurfaceError as _SurfaceError;

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAdapter(AdapterKind);

    impl Adapter for DummyAdapter {
        fn kind(&self) -> AdapterKind {
            self.0
        }

        fn create_surface(
            &self,
            _request: &RenderRequest,
        ) -> Result<Box<dyn Surface>, AdapterError> {
            Err(AdapterError::UnsupportedFormat(self.0, OutputFormat::Json))
        }
    }

    #[test]
    fn round_trip_kind() {
        for kind in AdapterKind::all() {
            assert_eq!(AdapterKind::parse(kind.as_str()).unwrap(), *kind);
        }
    }

    #[test]
    fn registry_returns_unregistered_for_missing_kind() {
        let reg = AdapterRegistry::new();
        assert!(matches!(
            reg.get(AdapterKind::Server),
            Err(AdapterError::Unregistered(AdapterKind::Server))
        ));
    }

    #[test]
    fn registry_returns_registered_adapter() {
        let mut reg = AdapterRegistry::new();
        reg.register(Arc::new(DummyAdapter(AdapterKind::Server)));
        let adapter = reg.get(AdapterKind::Server).unwrap();
        assert_eq!(adapter.kind(), AdapterKind::Server);
    }
}
