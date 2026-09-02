# Changelog

All notable changes to **seed-canvas** are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Core deterministic seed manager with SHA-256 domain-separated derivation.
- `Seed` API supporting `u64`, `f32`, `range`, `weighted`, `branch`, and `fork`.
- `Template`, `Artwork`, and `Gallery` data models with JSON Schema parameter validation.
- `ServerAdapter` (CPU rasterization via `@napi-rs/canvas`) and `SvgAdapter` (declarative vector output).
- Reference `galaxy` template (particle nebula) demonstrating the full pipeline.
- CLI commands: `init`, `render`, `share`, `list`, `random`, `verify`, `doctor`.
- Golden-sample snapshot tests for cross-platform determinism.
- Documentation site (Starlight) scaffold with user, author, and self-hosting guides.
- GitHub Actions CI: lint, typecheck, test, and golden-image diff across Ubuntu/macOS/Windows.

### Notes
- This is the M1 milestone release. Web UI, registry, embed, and editor land in M2-M5.

[Unreleased]: https://github.com/seed-canvas/seed-canvas/compare/v0.0.0...HEAD