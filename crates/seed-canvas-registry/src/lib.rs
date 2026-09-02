//! Template registry for seed-canvas.
//!
//! A registry is a JSON index describing available templates. The
//! official index ships with the repository (`registry/index.json`) and
//! can also be fetched from a URL by [`RegistryClient`].
//!
//! ## Why manifests, not code
//!
//! Templates in seed-canvas are compiled Rust entry functions; the
//! registry carries their **metadata** (id, version, license, tags).
//! This keeps the format stable while the distribution mechanism
//! (built-in today, WASM packages later) evolves. Third-party template
//! distribution is tracked in the project roadmap.
//!
//! ## Format
//!
//! ```json
//! {
//!   "schemaVersion": 1,
//!   "name": "…",
//!   "templates": [
//!     {"id": "galaxy", "version": "0.1.0", "license": "MIT", …}
//!   ]
//! }
//! ```

#![deny(missing_docs)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current index schema version understood by this crate.
pub const SCHEMA_VERSION: u64 = 1;

/// Default fetch timeout for remote registries.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors raised by the registry layer.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// JSON parsing or validation failed.
    #[error("registry index invalid: {0}")]
    InvalidIndex(String),

    /// Remote fetch failed.
    #[error("fetch failed: {0}")]
    Fetch(String),

    /// The requested template is not in this registry.
    #[error("template not found: {0}")]
    NotFound(String),

    /// I/O failure while reading or writing the cache.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// One template entry in a registry index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateEntry {
    /// Stable lowercase identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// One-sentence description.
    #[serde(default)]
    pub description: String,
    /// Latest semver.
    pub version: String,
    /// Author display names.
    #[serde(default)]
    pub authors: Vec<String>,
    /// SPDX license identifier.
    pub license: String,
    /// Gallery tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// True when the template ships inside the seed-canvas binary.
    #[serde(default)]
    pub builtin: bool,
    /// Minimum seed-canvas version required.
    #[serde(default = "default_min")]
    pub min_seed_canvas: String,
}

fn default_min() -> String {
    "0.1.0".to_owned()
}

/// A parsed registry index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemplateIndex {
    /// Schema version; must equal [`SCHEMA_VERSION`].
    #[serde(rename = "schemaVersion")]
    pub schema_version: u64,
    /// Human-readable registry name.
    #[serde(default)]
    pub name: String,
    /// Canonical URL of this index, when remote.
    #[serde(default)]
    pub url: Option<String>,
    /// RFC 3339 timestamp of the last update.
    #[serde(default)]
    pub updated: Option<String>,
    /// All listed templates.
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

impl TemplateIndex {
    /// Parse and validate an index from JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::InvalidIndex`] on malformed JSON, a
    /// schema-version mismatch, or duplicate template ids.
    pub fn parse(bytes: &[u8]) -> Result<Self, RegistryError> {
        let index: Self = serde_json::from_slice(bytes)
            .map_err(|e| RegistryError::InvalidIndex(e.to_string()))?;
        index.validate()?;
        Ok(index)
    }

    /// Validate structural invariants.
    ///
    /// # Errors
    ///
    /// See [`Self::parse`].
    pub fn validate(&self) -> Result<(), RegistryError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(RegistryError::InvalidIndex(format!(
                "schemaVersion {} unsupported (expected {})",
                self.schema_version, SCHEMA_VERSION
            )));
        }
        let mut seen = std::collections::HashSet::new();
        for t in &self.templates {
            if t.id.is_empty() || t.id != t.id.to_lowercase() {
                return Err(RegistryError::InvalidIndex(format!(
                    "template id {:?} must be non-empty lowercase",
                    t.id
                )));
            }
            if !seen.insert(t.id.clone()) {
                return Err(RegistryError::InvalidIndex(format!(
                    "duplicate template id {:?}",
                    t.id
                )));
            }
            if t.license.is_empty() {
                return Err(RegistryError::InvalidIndex(format!(
                    "template {:?} is missing a license",
                    t.id
                )));
            }
        }
        Ok(())
    }

    /// Look up a template by id.
    pub fn get(&self, id: &str) -> Option<&TemplateEntry> {
        self.templates.iter().find(|t| t.id == id)
    }
}

/// The built-in index embedded in the binary, parsed from the
/// repository's `registry/index.json`.
///
/// # Errors
///
/// Returns [`RegistryError::InvalidIndex`] if the embedded index is
/// malformed — which would be a build-time bug, not a runtime condition.
pub fn builtin_index() -> Result<TemplateIndex, RegistryError> {
    TemplateIndex::parse(include_bytes!("../../../registry/index.json"))
}

/// Client for fetching remote registry indexes with a local cache.
pub struct RegistryClient {
    cache_dir: PathBuf,
    timeout: Duration,
}

impl RegistryClient {
    /// Construct a client caching under `cache_dir`.
    #[must_use]
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            timeout: FETCH_TIMEOUT,
        }
    }

    /// Override the fetch timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Fetch a remote index, validating it before caching.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Fetch`] on network failure and
    /// [`RegistryError::InvalidIndex`] when the response is malformed.
    pub fn fetch(&self, url: &str) -> Result<TemplateIndex, RegistryError> {
        use std::io::Read as _;
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let response = agent
            .get(url)
            .call()
            .map_err(|e| RegistryError::Fetch(e.to_string()))?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(16 * 1024 * 1024) // 16 MiB cap — indexes are tiny
            .read_to_end(&mut bytes)
            .map_err(|e| RegistryError::Fetch(e.to_string()))?;
        let index = TemplateIndex::parse(&bytes)?;
        Ok(index)
    }

    /// Fetch a remote index and cache it under the cache directory.
    /// Returns the parsed index.
    ///
    /// # Errors
    ///
    /// Same as [`Self::fetch`], plus I/O errors while writing the cache.
    pub fn fetch_and_cache(&self, url: &str) -> Result<TemplateIndex, RegistryError> {
        let index = self.fetch(url)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        let cache_path = self.cache_path_for(url);
        std::fs::write(
            cache_path,
            serde_json::to_vec_pretty(&index)
                .map_err(|e| RegistryError::InvalidIndex(e.to_string()))?,
        )?;
        Ok(index)
    }

    /// Load a previously cached index by its URL. Returns `None` when
    /// nothing is cached.
    pub fn cached(&self, url: &str) -> Option<TemplateIndex> {
        let path = self.cache_path_for(url);
        let bytes = std::fs::read(path).ok()?;
        TemplateIndex::parse(&bytes).ok()
    }

    fn cache_path_for(&self, url: &str) -> PathBuf {
        // Stable, filesystem-safe key: hash the URL.
        let digest = seed_canvas_core::hash::sha256_hex(&[url.as_bytes()]);
        self.cache_dir.join(format!("registry-{digest}.json"))
    }
}

/// List ids of templates compiled into the running binary. Used by the
/// CLI to distinguish "installed" from "available".
#[must_use]
pub fn builtin_template_ids() -> &'static [&'static str] {
    &["galaxy", "particles", "mandala"]
}

/// Load an index from a local file path.
///
/// # Errors
///
/// I/O or parse errors surface as [`RegistryError`].
pub fn load_from_path(path: &Path) -> Result<TemplateIndex, RegistryError> {
    let bytes = std::fs::read(path)?;
    TemplateIndex::parse(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "schemaVersion": 1,
        "name": "test",
        "templates": [
            {"id": "galaxy", "name": "Galaxy", "version": "0.1.0", "license": "MIT"}
        ]
    }"#;

    #[test]
    fn parses_valid_index() {
        let idx = TemplateIndex::parse(VALID.as_bytes()).unwrap();
        assert_eq!(idx.templates.len(), 1);
        assert_eq!(idx.get("galaxy").unwrap().name, "Galaxy");
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let bad = VALID.replace("\"schemaVersion\": 1", "\"schemaVersion\": 99");
        assert!(matches!(
            TemplateIndex::parse(bad.as_bytes()),
            Err(RegistryError::InvalidIndex(_))
        ));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let dup = r#"{
            "schemaVersion": 1,
            "templates": [
                {"id": "x", "name": "X", "version": "0.1.0", "license": "MIT"},
                {"id": "x", "name": "X2", "version": "0.2.0", "license": "MIT"}
            ]
        }"#;
        assert!(TemplateIndex::parse(dup.as_bytes()).is_err());
    }

    #[test]
    fn rejects_uppercase_ids() {
        let bad = VALID.replace("\"id\": \"galaxy\"", "\"id\": \"Galaxy\"");
        assert!(TemplateIndex::parse(bad.as_bytes()).is_err());
    }

    #[test]
    fn rejects_missing_license() {
        let bad = r#"{
            "schemaVersion": 1,
            "templates": [{"id": "x", "name": "X", "version": "0.1.0"}]
        }"#;
        assert!(TemplateIndex::parse(bad.as_bytes()).is_err());
    }

    #[test]
    fn builtin_index_parses_and_lists_three() {
        let idx = builtin_index().unwrap();
        assert_eq!(idx.templates.len(), 3);
        for id in builtin_template_ids() {
            assert!(idx.get(id).is_some(), "builtin index missing {id}");
        }
    }

    #[test]
    fn cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let client = RegistryClient::new(dir.path());
        let url = "https://example.com/index.json";
        // Seed the cache manually (no network in tests).
        let cache_path = {
            let digest = seed_canvas_core::hash::sha256_hex(&[url.as_bytes()]);
            dir.path().join(format!("registry-{digest}.json"))
        };
        std::fs::write(&cache_path, VALID).unwrap();
        let cached = client.cached(url).expect("cache hit");
        assert_eq!(cached.get("galaxy").unwrap().version, "0.1.0");
    }
}
