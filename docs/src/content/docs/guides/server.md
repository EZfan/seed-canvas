---
title: Gallery Server
description: One command turns any machine into a deterministic-art gallery.
---

`seed-canvas serve` starts a self-hosted gallery. No JavaScript
framework, no build step — server-rendered HTML with vanilla CSS that
respects `prefers-color-scheme`.

```bash
seed-canvas serve --addr 0.0.0.0:8080 --root ~/gallery
```

## Routes

| Route | What it is |
| --- | --- |
| `/` | Gallery index — recent artworks, template list |
| `/p/:template/:seed` | Artwork detail: image, download buttons, copy-URL, metadata |
| `/t/:template` | Template page with its rendered artworks |
| `/art/:template/:seed.(png\|svg\|json)` | Raw artifact bytes |
| `/embed/:template/:seed` | Minimal page for `<iframe>` embedding |
| `/og/:template/:seed` | 1200×630 share image for Open Graph / Twitter cards |
| `/api/render` | `POST {template, seed, format, params?}` → content hash + URL |
| `/api/artworks` | Recent artworks as JSON |
| `/api/search?q=…` | FTS5 full-text search |

## Every render is stored

Both `POST /api/render` and the artwork routes persist what they render
into the workspace SQLite database, so the gallery fills up as people
browse.

## Graceful shutdown

SIGINT/SIGTERM drain connections before exit — safe under systemd,
Docker, or Kubernetes.