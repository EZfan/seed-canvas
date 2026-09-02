//! Server-wide state held by every handler.
//.
//!! Server-wide state held by every handler.

use std::path::PathBuf;
use std::sync::Arc;

use seed_canvas_core::adapter::{Adapter, AdapterRegistry};
use seed_canvas_core::template::{
    Params, RenderContext, Template, TemplateError, TemplateManifest,
};
use seed_canvas_core::Seed;
use seed_canvas_storage::Gallery;

/// Trait-object-aware newtype for templates. Each installed template is
/// wrapped in an `Arc<dyn ErasedRender>` so the server can iterate over
/// them without knowing their concrete types.
pub type BoxedTemplate = Arc<dyn ErasedRender>;

/// Minimal surface so the server can ask a template to render without
/// knowing its concrete type. Each implementation is a thin shim over a
/// concrete [`Template`] built by an `examples/*` crate.
pub trait ErasedRender: Send + Sync {
    /// Manifest accessor — borrowed reference to the template's static
    /// metadata.
    fn manifest(&self) -> &TemplateManifest;

    /// Validate `params` against the template's JSON Schema and return a
    /// frozen [`Params`].
    fn validate(&self, params: serde_json::Value) -> Result<Params, TemplateError>;

    /// Run the template's entry function with the supplied seed stream,
    /// writing to `surface`. Assumes the caller has already validated
    /// `params` via [`Self::validate`].
    fn render_into(
        &self,
        seed: &mut Seed,
        params: &Params,
        surface: &mut dyn seed_canvas_core::Surface,
    ) -> Result<(), TemplateError>;
}

// `galaxy::Template` is a type alias for `seed_canvas_core::template::Template`,
// so we get `ErasedRender` for free.
impl ErasedRender for Template {
    fn manifest(&self) -> &TemplateManifest {
        Template::manifest(self)
    }

    fn validate(&self, params: serde_json::Value) -> Result<Params, TemplateError> {
        Template::validate_params(self, params)
    }

    fn render_into(
        &self,
        seed: &mut Seed,
        params: &Params,
        surface: &mut dyn seed_canvas_core::Surface,
    ) -> Result<(), TemplateError> {
        Template::render(self, seed, params, surface)
    }
}

/// Server state shared by every axum handler.
#[derive(Clone)]
pub struct ServerState {
    /// Persistent storage layer (artworks + FTS5 index).
    pub gallery: Arc<Gallery>,
    /// Adapter registry with server + svg adapters registered.
    pub registry: Arc<AdapterRegistry>,
    /// Installed templates keyed by their lowercase id.
    pub templates: Arc<std::sync::Mutex<Vec<BoxedTemplate>>>,
    /// Root path the server is serving from. Used to resolve static
    /// assets that live next to the binary.
    pub root: PathBuf,
}

impl ServerState {
    /// Construct a fresh `ServerState` rooted at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let gallery = Gallery::open(&root).expect("gallery.db open must succeed");
        let mut registry = AdapterRegistry::new();
        registry.register(
            Arc::new(seed_canvas_adapter_server::ServerAdapter::new()) as Arc<dyn Adapter>
        );
        registry.register(Arc::new(seed_canvas_adapter_svg::SvgAdapter::new()) as Arc<dyn Adapter>);
        let templates: Vec<BoxedTemplate> = vec![Arc::new(galaxy::build())];
        Self {
            gallery: Arc::new(gallery),
            registry: Arc::new(registry),
            templates: Arc::new(std::sync::Mutex::new(templates)),
            root,
        }
    }

    /// Register an additional template. Used by `examples/particles` /
    /// `examples/mandala` once those land.
    pub fn register_template<T: ErasedRender + 'static>(&self, template: Arc<T>) {
        let mut guard = self.templates.lock().expect("templates mutex poisoned");
        guard.push(template as BoxedTemplate);
    }

    /// Resolve a template by its id. Returns `None` for unknown ids so
    /// the HTTP layer can emit a 404.
    #[must_use]
    pub fn template(&self, id: &str) -> Option<BoxedTemplate> {
        let guard = self.templates.lock().expect("templates mutex poisoned");
        guard.iter().find(|t| t.manifest().id == id).cloned()
    }

    /// List all installed template manifests.
    #[must_use]
    pub fn list_templates(&self) -> Vec<TemplateManifest> {
        let guard = self.templates.lock().expect("templates mutex poisoned");
        guard.iter().map(|t| t.manifest().clone()).collect()
    }
}

#[doc(hidden)]
pub use _unused::*;
#[doc(hidden)]
mod _unused {
    // Force the linker to keep `RenderContext` available for trait
    // implementations that reference it (none do today, but this keeps
    // the import honest if the trait shape changes).
    use super::*;
    pub type Ctx<'a> = RenderContext<'a>;
}
