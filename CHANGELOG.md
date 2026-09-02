# Changelog

All notable changes to **seed-canvas** are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Core deterministic seed manager with SHA-256 domain-separated derivation
  and named sub-stream forks (`seed.fork("color")`).
- `Seed` API: `next_u64`, `range`, `f64`, `weighted`, `branch`, `fork`, and
  content-addressed `sc_…` handles.
- `Template` registry with JSON-Schema parameter validation; templates may
  render at any canvas size via `RenderRequest::size_override`.
- Adapters: `server` (tiny-skia PNG rasterizer) and `svg` (declarative
  vector documents), both byte-deterministic across platforms.
- Built-in templates: `galaxy` (spiral nebula), `particles` (flow-field
  ribbons), `mandala` (symmetric petal rings).
- CLI: `init`, `render`, `share`, `list`, `random`, `verify`, `doctor`,
  `url`, `serve`, `export`, `embed`, `registry`, `install`.
- Self-hosted gallery server (axum): HTML gallery, artwork bytes, JSON
  API, FTS5 search, iframe-friendly embed pages, and 1200×630 Open Graph
  share images rendered through the same deterministic pipeline.
- Self-contained HTML export — artworks embedded as `data:` URLs, renders
  identically offline.
- Template registry client: validated remote indexes (HTTPS + loopback
  http for development), local caching, `registry add/list/remove`.
- SQLite storage layer with FTS5 full-text search and connection pooling.
- Documentation site (Astro Starlight) deployed to GitHub Pages.
- Docker image (multi-stage, non-root, ~20 MB) with compose file.
- GitHub Actions CI: rustfmt, clippy, tests, golden-sample determinism,
  cargo audit, docker build, and rustdoc across Linux/macOS/Windows.

### Fixed
- Concurrent gallery writers no longer fail with `database is locked`
  (SQLite `busy_timeout`).
- `/api/search` quotes queries as FTS5 phrases, so seeds containing `-`
  are matched literally instead of raising a 500.

### Notes
- This is a pre-1.0 development release; seed derivation, parameter
  schemas, and the storage schema may still change before 1.0.

[Unreleased]: https://github.com/EZfan/seed-canvas/compare/v0.1.0...HEAD