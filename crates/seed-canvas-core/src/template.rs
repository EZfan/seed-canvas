//! Template manifests and runtime validation.
//!
//! A template is a manifest (the JSON-described metadata) plus an entry
//! function (the actual generator). The manifest is loaded from disk at
//! install time; the entry function is compiled into the binary.

use crate::surface::Surface;
use crate::Seed;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Author of a template. Mirrors the [`Cargo.toml` `authors`] field but is
/// JSON-friendly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Author {
    /// Display name.
    pub name: String,
    /// Optional homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Optional contact email. Public galleries should not surface this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// JSON Schema (draft 2020-12) describing a template's accepted `params`.
/// We deliberately type-erase the schema to a `serde_json::Value` so
/// adapters and front-ends can introspect it without depending on
/// `schemars`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParamsSchema(pub serde_json::Value);

impl ParamsSchema {
    /// Construct from any JSON-Schema-compatible value. Returns an error if
    /// `value` is not a JSON object.
    pub fn new(value: serde_json::Value) -> Result<Self, TemplateError> {
        if !value.is_object() {
            return Err(TemplateError::InvalidSchema(
                "schema must be a JSON object".into(),
            ));
        }
        Ok(Self(value))
    }

    /// Borrow the underlying JSON value.
    #[must_use]
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }
}

/// The static metadata describing a template.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TemplateManifest {
    /// Stable lowercase identifier, e.g. `"galaxy"`.
    pub id: String,
    /// Display name, e.g. `"Galaxy"`.
    pub name: String,
    /// Semantic version, e.g. `"1.2.0"`.
    pub version: String,
    /// One-sentence description used by galleries and registries.
    pub description: String,
    /// Author(s) of the template.
    pub authors: Vec<Author>,
    /// SPDX license identifier, e.g. `"MIT"`.
    pub license: String,
    /// Recommended canvas size.
    pub canvas: CanvasSize,
    /// Tags used by galleries' search UI.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Minimum `seed-canvas` version this template requires.
    pub min_seed_canvas: String,
    /// JSON Schema describing accepted params.
    pub params_schema: serde_json::Value,
    /// Adapters this template supports.
    pub adapters: Vec<AdapterKind>,
    /// Relative path to a 1:1 thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

/// Recommended canvas size in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CanvasSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Validated, frozen parameter object passed to a template entry.
pub type Params = serde_json::Map<String, serde_json::Value>;

/// Re-exported here so adapters can serialize/deserialize without an extra
/// `use`.
pub use crate::adapter::AdapterKind;

/// Bundle passed to a template's entry function. Keep this surface small
/// so templates stay portable.
pub struct RenderContext<'a> {
    /// Deterministic seed stream.
    pub seed: &'a mut Seed,
    /// Validated, frozen parameters.
    pub params: &'a Params,
    /// Surface to draw onto.
    pub surface: &'a mut dyn Surface,
    /// Canvas size.
    pub canvas: CanvasSize,
}

/// Pure entry function every template implements.
///
/// A template MUST be a pure function: the same `(seed, params)` must
/// call the same `Surface` methods in the same order. Adapters and the
/// `verify` CLI depend on this contract.
pub type TemplateEntry = fn(&mut RenderContext<'_>) -> Result<(), TemplateError>;

/// A registered template, pairing its manifest with its compiled entry.
#[derive(Debug)]
pub struct Template {
    manifest: TemplateManifest,
    entry: TemplateEntry,
}

impl Template {
    /// Construct a new template from its manifest + entry.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError`] if the manifest is malformed (non-lowercase
    /// id, non-semver version, missing required fields, etc.).
    pub fn new(manifest: TemplateManifest, entry: TemplateEntry) -> Result<Self, TemplateError> {
        if manifest.id.to_lowercase() != manifest.id {
            return Err(TemplateError::InvalidManifest(format!(
                "id must be lowercase, got {:?}",
                manifest.id
            )));
        }
        validate_semver(&manifest.version)?;
        validate_semver(&manifest.min_seed_canvas)?;
        if manifest.canvas.width == 0 || manifest.canvas.height == 0 {
            return Err(TemplateError::InvalidManifest(
                "canvas dimensions must be positive".into(),
            ));
        }
        if manifest.adapters.is_empty() {
            return Err(TemplateError::InvalidManifest(
                "template must declare at least one adapter".into(),
            ));
        }
        Ok(Self { manifest, entry })
    }

    /// Borrow the manifest.
    #[must_use]
    pub fn manifest(&self) -> &TemplateManifest {
        &self.manifest
    }

    /// Validate `params` against this template's schema. Returns a frozen
    /// map suitable for passing to the entry function.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::InvalidParams`] when the JSON Schema
    /// validator rejects `params`. The error message includes the failing
    /// instance path.
    pub fn validate_params(&self, params: serde_json::Value) -> Result<Params, TemplateError> {
        let schema = ParamsSchema::new(self.manifest.params_schema.clone())?;
        let compiled = jsonschema::validator_for(&schema.0)
            .map_err(|e| TemplateError::InvalidSchema(format!("compile failed: {e}")))?;
        compiled
            .validate(&params)
            .map_err(|err| TemplateError::InvalidParams(err.to_string()))?;
        let object = params
            .as_object()
            .ok_or_else(|| TemplateError::InvalidParams("params must be a JSON object".into()))?;
        Ok(object.clone())
    }

    /// Run the template against `seed` + `params` and write to `surface`.
    ///
    /// # Errors
    ///
    /// Returns the error produced by the entry function, or any error
    /// surfaced from the adapter.
    pub fn render(
        &self,
        seed: &mut Seed,
        params: &Params,
        surface: &mut dyn Surface,
    ) -> Result<(), TemplateError> {
        self.render_with_canvas(seed, params, surface, self.manifest.canvas)
    }

    /// The template's default canvas size.
    #[must_use]
    pub const fn canvas_dimensions(&self) -> CanvasSize {
        self.manifest.canvas
    }

    /// Like [`Self::render`] but with an explicit canvas size. The
    /// template must scale its geometry from `ctx.canvas`. Used for OG
    /// images, thumbnails, and other size-overridden renders.
    ///
    /// # Errors
    ///
    /// Returns the error produced by the entry function.
    pub fn render_with_canvas(
        &self,
        seed: &mut Seed,
        params: &Params,
        surface: &mut dyn Surface,
        canvas: CanvasSize,
    ) -> Result<(), TemplateError> {
        let mut ctx = RenderContext {
            seed,
            params,
            surface,
            canvas,
        };
        (self.entry)(&mut ctx)
    }
}

/// Errors raised by templates and their validation pipeline.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// `template.toml` failed to parse.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    /// `params_schema` is not a valid JSON Schema.
    #[error("invalid schema: {0}")]
    InvalidSchema(String),

    /// `params` failed JSON Schema validation.
    #[error("invalid params: {0}")]
    InvalidParams(String),

    /// Template entry function returned an error.
    #[error("render failed: {0}")]
    Render(String),

    /// A required adapter is not installed.
    #[error("adapter unavailable: {0:?}")]
    AdapterUnavailable(AdapterKind),
}

/// Minimal semver validator. Accepts `MAJOR.MINOR.PATCH` with optional
/// pre-release / build metadata. Does not perform precedence comparison;
/// that is delegated to `semver` (only at install time).
fn validate_semver(s: &str) -> Result<(), TemplateError> {
    let mut iter = s.split('.');
    let major = iter
        .next()
        .ok_or_else(|| TemplateError::InvalidManifest(format!("version {s:?} missing major")))?;
    let minor = iter
        .next()
        .ok_or_else(|| TemplateError::InvalidManifest(format!("version {s:?} missing minor")))?;
    let rest = iter
        .next()
        .ok_or_else(|| TemplateError::InvalidManifest(format!("version {s:?} missing patch")))?;
    if iter.next().is_some() {
        return Err(TemplateError::InvalidManifest(format!(
            "version {s:?} has too many components"
        )));
    }
    let patch = rest.split('-').next().unwrap_or(rest);
    for (name, part) in [("major", major), ("minor", minor), ("patch", patch)] {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(TemplateError::InvalidManifest(format!(
                "version {s:?}: {name} component not numeric"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_manifest() -> TemplateManifest {
        TemplateManifest {
            id: "test".into(),
            name: "Test".into(),
            version: "0.1.0".into(),
            description: "A test template".into(),
            authors: vec![Author {
                name: "Anon".into(),
                url: None,
                email: None,
            }],
            license: "MIT".into(),
            canvas: CanvasSize {
                width: 800,
                height: 600,
            },
            tags: vec!["test".into()],
            min_seed_canvas: "0.1.0".into(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "minimum": 1}
                }
            }),
            adapters: vec![AdapterKind::Server, AdapterKind::Svg],
            thumbnail: None,
        }
    }

    fn dummy_entry(_ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
        Ok(())
    }

    #[test]
    fn rejects_uppercase_id() {
        let mut m = dummy_manifest();
        m.id = "Galaxy".into();
        assert!(Template::new(m, dummy_entry).is_err());
    }

    #[test]
    fn rejects_non_semver_version() {
        let mut m = dummy_manifest();
        m.version = "v1".into();
        assert!(Template::new(m, dummy_entry).is_err());
    }

    #[test]
    fn rejects_zero_canvas() {
        let mut m = dummy_manifest();
        m.canvas.width = 0;
        assert!(Template::new(m, dummy_entry).is_err());
    }

    #[test]
    fn rejects_empty_adapter_list() {
        let mut m = dummy_manifest();
        m.adapters.clear();
        assert!(Template::new(m, dummy_entry).is_err());
    }

    #[test]
    fn params_schema_is_generated_for_manifest() {
        // Smoke test: TemplateManifest implements JsonSchema (compile-time
        // assertion via the trait bound on JsonSchema).
        fn assert_json_schema<T: JsonSchema>() {}
        assert_json_schema::<TemplateManifest>();
    }

    #[test]
    fn params_validation_succeeds_for_valid_input() {
        let tpl = Template::new(dummy_manifest(), dummy_entry).unwrap();
        let params = serde_json::json!({"count": 100});
        tpl.validate_params(params).expect("valid params");
    }

    #[test]
    fn params_validation_rejects_bad_input() {
        let tpl = Template::new(dummy_manifest(), dummy_entry).unwrap();
        let params = serde_json::json!({"count": 0});
        assert!(tpl.validate_params(params).is_err());
    }
}
