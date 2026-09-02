---
title: CLI Reference
description: Every seed-canvas subcommand, flag by flag.
---

`seed-canvas` ships one binary with the following subcommands. Global
flags: `-v/-vv/-vvv` raises log verbosity.

| Command | Purpose |
| --- | --- |
| `init [dir]` | Create a gallery workspace (`seed-canvas.toml`, `templates/`, `artworks/`) |
| `render` | Render one artwork to a file |
| `share` | Render + write to disk + print the canonical URL |
| `list` | Show installed templates and registered adapters |
| `random` | Print a fresh entropy-backed seed |
| `verify` | Re-render and print (or check) the content hash |
| `doctor` | Environment diagnostics |
| `url` | Print the canonical URL for a seed/template pair |
| `serve` | Run the self-hosted gallery server |
| `export` | Self-contained HTML export (artwork embedded as `data:` URL) |
| `embed` | Print an `<iframe>` snippet for a running server |
| `registry` | Manage remote template registries (`list` / `add` / `remove`) |
| `install` | Look up a template across registries |

## render

```bash
seed-canvas render --template <id> --seed <s> \
    [--format png|svg|json] [--out path] [--params '{}']
```

## serve

```bash
seed-canvas serve [--addr 127.0.0.1:8080] [--root .]
```

Binds a graceful-shutdown HTTP server with gallery pages, artwork
bytes, a JSON API, `/og` share images, and iframe-friendly embed pages.

## export

```bash
seed-canvas export --template galaxy --seed cosmos --out page.html
seed-canvas export --all --root ./gallery --title "My Gallery" --out gallery.html
```

The output is a single HTML file with every image inlined as a `data:`
URL — it renders identically offline, in email, or from a USB stick.

## registry

```bash
seed-canvas registry list
seed-canvas registry add https://example.com/seed-canvas-index.json
seed-canvas registry remove <url>
```

Remote URLs must use HTTPS (loopback `http://` is allowed for local
development). Indexes are validated (schema version, unique ids, SPDX
licenses) and cached under your config directory.