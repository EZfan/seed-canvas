---
title: Registry Index
description: Format of registry/index.json — normative reference.
---

A registry is a JSON document listing template metadata.

```json
{
  "schemaVersion": 1,
  "name": "seed-canvas official templates",
  "url": "https://example.com/index.json",
  "updated": "2026-09-02T00:00:00Z",
  "templates": [
    {
      "id": "galaxy",
      "name": "Galaxy",
      "description": "…",
      "version": "0.1.0",
      "authors": ["seed-canvas contributors"],
      "license": "MIT",
      "tags": ["space", "nebula"],
      "builtin": true,
      "minSeedCanvas": "0.1.0"
    }
  ]
}
```

## Validation rules

- `schemaVersion` must be `1`.
- `id` must be non-empty lowercase; ids are unique.
- `license` must be a non-empty SPDX expression.
- Remote indexes are capped at 16 MiB and cached under the user config
  directory keyed by the SHA-256 of their URL.

## CLI

```bash
seed-canvas registry list                 # built-in + configured remotes
seed-canvas registry add <https-url>      # fetch, validate, cache, remember
seed-canvas registry remove <url>
seed-canvas install <template-id>         # look up across registries
```

Loopback `http://` URLs are allowed for local development; everything
else must be HTTPS.

## Distribution status

Templates today are compiled Rust entry functions — the registry
carries their metadata. Binary template packages (WASM) are on the
roadmap; the index format above is designed to extend with an
`artifact` field without a breaking change.