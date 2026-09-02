---
title: Seed Format
description: The exact derivation behind a seed handle — normative reference.
---

## Derivation

```
bytes   = SHA-256( domain_tag || 0x00 || raw_utf8 )
state₀  = LE64(bytes[0..8])
next()  = SplitMix64: state += 0x9E3779B97F4A7C15;
                        z = state; z = (z ^ z>>30) * 0xBF58476D1CE4E5B9;
                        z = (z ^ z>>27) * 0x94D049BB133111EB;
                        return z ^ z>>31;
```

- `domain_tag` defaults to `seed-canvas/v1`.
- `handle` = `"sc_"` + hex(`bytes[0..12]`) — 96 bits, 27 characters,
  URL-safe.

## Sampling rules

| API | Definition |
| --- | --- |
| `f64(lo, hi)` | `lo + (hi-lo) * (next() >> 11) / 2^53` — uniform, exactly representable |
| `range(n)` | Lemire bounded multiply — uniform, no modulo bias |
| `weighted(pairs)` | Inverse-CDF over normalized weights |
| `branch(p)` | `f64(0,1) < p` |
| `fork(label)` | `mix64(state+φ) XOR mix64(LE64(SHA256(domain‖label)[0..8])))` |

## Versioning

The domain tag changes when the derivation changes
(`seed-canvas/v2`, …), so historical seeds keep rendering identically
under the old scheme. Adapters record which engine produced an
artwork.