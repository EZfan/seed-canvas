---
title: Quickstart
description: Render your first deterministic artwork in under a minute.
---

## Install

Requires Rust ≥ 1.80.

```bash
cargo install seed-canvas-cli
```

## Render

```bash
seed-canvas render --template galaxy --seed cosmos --out cosmos.png
```

Open `cosmos.png`. Now render the same seed again — the bytes are
identical. That is the whole point: **the seed is the artwork**.

## Share

```bash
seed-canvas share --template galaxy --seed cosmos --out cosmos.png
# writes cosmos.png and prints a canonical URL:
# https://art.example.com/p/galaxy/sc_1eebd7175c6b0b26921647f4
```

## Browse

```bash
seed-canvas serve                # http://127.0.0.1:8080
```

## Verify determinism

`verify` re-renders and prints the SHA-256 of the output. Wire it into
CI to catch renderer regressions:

```bash
seed-canvas verify --template galaxy --seed cosmos
# 3fab6d24ddbd80510637d171efe974e24851f3326aa60605068c88a8ebb05171
```

## Built-in templates

| Template | Description | Signature params |
| --- | --- | --- |
| `galaxy` | Spiral galaxy with nebula haze | `count`, `arms`, `windings`, `nebula` |
| `particles` | Colored flow-field ribbons | `count`, `trail_length`, `background` |
| `mandala` | Symmetric petal rings | `symmetry`, `rings`, `background` |

Pass params as JSON:

```bash
seed-canvas render --template galaxy --seed cosmos \
  --params '{"arms": 3, "count": 4000}' --out triskelion.png
```