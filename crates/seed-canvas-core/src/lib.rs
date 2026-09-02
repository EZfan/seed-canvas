//! # seed-canvas-core
//!
//! The deterministic engine that powers every seed-canvas render.
//!
//! This crate exposes three primitives:
//!
//! * [`Seed`] — a deterministic 64-bit stream derived from a human-readable
//!   string. The same `(raw, domain_tag)` always produces the same stream, on
//!   every platform, with no allocations after construction.
//! * [`Template`] — a validated, pure function from `(seed, params)` to
//!   surface calls. Templates are the only sanctioned way to author artwork.
//! * [`Surface`] — an abstract canvas that adapters translate into PNG, SVG,
//!   WebGL, or any future backend.
//!
//! Determinism is enforced by:
//!
//! 1. SHA-256 domain separation (`seed-canvas/v1` default tag).
//! 2. SplitMix64 mixing of the derived state.
//! 3. Pure template functions — no I/O, no global state, no wall clock.
//!
//! The contract is small on purpose: small contracts are auditable, and
//! auditable contracts are what allow us to ship the `verify` CLI command.

#![deny(missing_docs)]

pub mod adapter;
pub mod hash;
pub mod render;
pub mod seed;
pub mod surface;
pub mod template;

pub use adapter::{Adapter, AdapterError, AdapterKind, BoxedAdapter};
pub use hash::{seed_bytes, sha256_hex, DEFAULT_DOMAIN_TAG};
pub use render::{render, RenderError, RenderOutput, RenderRequest};
pub use seed::Seed;
pub use surface::{Color, NamedColor, Surface, SurfaceError, Vec2, NAMED_COLORS};
pub use template::{Author, Params, ParamsSchema, Template, TemplateEntry, TemplateManifest};
