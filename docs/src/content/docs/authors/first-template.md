---
title: Your First Template
description: A template is one pure function — this page gets you from zero to rendered.
---

A template is a **pure function** from `(seed, params)` to surface
calls, plus a JSON-Schema manifest. This walkthrough builds `rings`,
the simplest possible layered template.

## 1. Create the crate

```bash
cargo new my-templates --lib
cd my-templates
cargo add seed-canvas-core serde_json
```

## 2. Write the manifest

```rust
use seed_canvas_core::template::{
    Author, CanvasSize, Template, TemplateEntry, TemplateManifest,
};

fn manifest() -> TemplateManifest {
    TemplateManifest {
        id: "rings".into(),
        name: "Rings".into(),
        version: "0.1.0".into(),
        description: "Concentric rings with seeded colors.".into(),
        authors: vec![Author { name: "you".into(), url: None, email: None }],
        license: "MIT".into(),
        canvas: CanvasSize { width: 1024, height: 1024 },
        tags: vec!["geometric".into()],
        min_seed_canvas: "0.1.0".into(),
        params_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "count": { "type": "integer", "minimum": 1, "maximum": 200, "default": 24 }
            }
        }),
        adapters: vec![AdapterKind::Server, AdapterKind::Svg],
        thumbnail: None,
    }
}
```

The schema is enforced at render time: unknown or out-of-range params
are rejected before your entry function runs.

## 3. Write the entry function

```rust
use seed_canvas_core::surface::{Color, Vec2};
use seed_canvas_core::Seed;

fn entry(ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
    let count = ctx.params.get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(24) as usize;
    let (w, h) = (ctx.canvas.width as f64, ctx.canvas.height as f64);

    ctx.surface.clear(Color::rgb(0.02, 0.02, 0.05));

    // Fork a named sub-stream: tweaks to colors never reshuffle radii.
    let mut radii = ctx.seed.fork("radii");
    let mut colors = ctx.seed.fork("colors");

    for i in 0..count {
        let radius = (i + 1) as f64 * w * 0.45 / count as f64;
        let c = colors.weighted(&[
            (Color::rgb(0.95, 0.85, 0.65), 3.0),
            (Color::rgb(0.45, 0.85, 0.95), 3.0),
        ]);
        ctx.surface.fill_circle(Vec2::new(w / 2.0, h / 2.0), radius, c);
    }
    Ok(())
}
```

**Never** call `std::time`, read files, or use `rand` — take every
number from `ctx.seed` or `ctx.params`. That is what makes
`seed-canvas verify` possible.

## 4. Render it

```rust
let template = Template::new(manifest(), entry as TemplateEntry)?;
```

Wire it into your binary the way `examples/galaxy` does, or copy
`examples/particles` as a starting point. Render, then inspect the
SVG — you can read exactly what your template drew.

## Rules of the road

1. Scale geometry from `ctx.canvas`, never hard-code 1024 — your
   template will be rendered at 1200×630 for OG images.
2. Prefer `seed.fork("name")` per visual concern so parameter tweaks
   are localized.
3. Keep the entry function `Send + Sync`-safe and panic-free; return
   errors instead.