---
title: Docker
description: Self-host the gallery in one container.
---

## Quick start

```bash
docker build -t seedcanvas/seed-canvas -f docker/Dockerfile .
docker run --rm -p 8080:8080 -v "$PWD/gallery:/data" seedcanvas/seed-canvas
```

Or with compose (from `docker/`):

```bash
docker compose up -d
# → http://localhost:8080
```

## What is in the image

- Multi-stage build: `rust:1.83-slim` builder → `debian:bookworm-slim`
  runtime. Final image ≈ 20 MB + ca-certificates.
- Runs as the non-root `seedcanvas` user; the workspace volume is
  `/data`.
- `HEALTHCHECK` runs `seed-canvas doctor` every 30 s.

## Reverse proxy

The server speaks plain HTTP; terminate TLS at your proxy. Example
Caddyfile:

```
art.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

## Backups

Everything lives in `/data`: `gallery.db` (SQLite, WAL mode) is the
source of truth. Snapshot the directory, or use
`sqlite3 gallery.db ".backup …"` for a hot copy.