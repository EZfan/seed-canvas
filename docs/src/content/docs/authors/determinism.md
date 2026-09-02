---
title: Determinism Rules
description: The exact guarantees seed-canvas makes, and what template authors must not do.
---

## What is guaranteed

1. **Same seed + template + params → same bytes.** Byte-identical PNGs
   across Linux, macOS, and Windows; identical SVG across platforms.
2. **Content addressing.** Every render's SHA-256 is stable, so
   `seed-canvas verify` detects drift and galleries deduplicate.
3. **Stable handles.** A seed's `sc_…` handle is derived from the
   domain-separated digest and never changes.

## How it works

- Seed bytes are `SHA-256(domain_tag || 0x00 || raw)`; the default tag
  is `seed-canvas/v1`.
- The stream is SplitMix64 — forward-only, platform-independent integer
  math. No floating point is involved in stream generation; floats are
  derived from 53-bit integers, which is exact.
- Sub-streams (`fork`) hash the label under the same domain tag, so
  sibling streams never interact.

## What breaks determinism (never do this)

| Violation | Why it breaks |
| --- | --- |
| `std::time::Instant` / `SystemTime` | Different on every machine |
| `rand` crate / OS entropy | Non-repeatable by definition |
| HashMap iteration order | Randomized per process |
| `f32` math | Excess precision differs across arches; use `f64` |
| Unsorted collection iteration | Order may differ between versions |
| Threading without ordering | Race on draw-call order |

## How we enforce it

- CI renders golden seeds on all three OSes and compares hashes.
- `seed-canvas verify` re-renders and compares against a recorded hash.
- The workspace clippy configuration denies `unsafe` code outright.