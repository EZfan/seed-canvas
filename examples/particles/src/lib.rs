//! `particles` example template — a flowing particle field where each
//! particle drifts along a deterministic noise field.

use seed_canvas_core::adapter::AdapterKind;
use seed_canvas_core::surface::{Color, Vec2};
use seed_canvas_core::template::{
    Author, CanvasSize, RenderContext, Template, TemplateEntry, TemplateError, TemplateManifest,
};
use seed_canvas_core::Seed;

/// Manifest describing this template.
#[must_use]
pub fn manifest() -> TemplateManifest {
    TemplateManifest {
        id: "particles".into(),
        name: "Particles".into(),
        version: "0.1.0".into(),
        description:
            "Deterministic particle field — each particle drifts along a noisy vector field.".into(),
        authors: vec![Author {
            name: "seed-canvas contributors".into(),
            url: Some("https://github.com/EZfan/seed-canvas".into()),
            email: None,
        }],
        license: "MIT".into(),
        canvas: CanvasSize {
            width: 1024,
            height: 1024,
        },
        tags: vec!["particles".into(), "flow".into(), "field".into()],
        min_seed_canvas: "0.1.0".into(),
        params_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "count": {
                    "type": "integer",
                    "minimum": 50,
                    "maximum": 50_000,
                    "default": 1500,
                    "description": "Number of particles in the field."
                },
                "trail_length": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 200,
                    "default": 30,
                    "description": "Number of trail segments behind each particle."
                },
                "background": {
                    "type": "string",
                    "enum": ["ink", "paper", "ocean"],
                    "default": "ink",
                    "description": "Background palette."
                }
            }
        }),
        adapters: vec![AdapterKind::Server, AdapterKind::Svg],
        thumbnail: None,
    }
}

/// Entry function: draws a flowing particle field.
pub fn entry(ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
    let params = ctx.params.clone();
    let count = params
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1500) as usize;
    let trail_length = params
        .get("trail_length")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(30) as usize;
    let background = params
        .get("background")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ink");

    let (w, h) = (ctx.canvas.width as f64, ctx.canvas.height as f64);

    // Background
    let bg_color = match background {
        "paper" => Color::rgb(0.97, 0.94, 0.88),
        "ocean" => Color::rgb(0.02, 0.05, 0.12),
        _ => Color::rgb(0.02, 0.02, 0.04), // "ink"
    };
    ctx.surface.clear(bg_color);

    let mut layout = ctx.seed.fork("layout");
    let mut color_stream = ctx.seed.fork("color");
    let mut angle_stream = ctx.seed.fork("angle");

    // Shared direction field: particles sample the field by position, so
    // nearby particles flow the same way and the whole composition reads
    // as one coherent current instead of stray hairs.
    let field_phase = angle_stream.f32(0.0, std::f64::consts::TAU);
    let field_scale = 0.0028 + angle_stream.f32(0.0, 0.003);

    let direction = |x: f64, y: f64, s: &mut Seed| -> f64 {
        // Pseudo-curl: sinusoidal field seeded once per artwork, varying
        // smoothly across the canvas.
        let a = (x * field_scale).sin() + (y * field_scale * 1.3).cos();
        let b = ((x + y) * field_scale * 0.7).sin();
        field_phase + 2.4 * (a + b) + s.f32(-0.18, 0.18) // small per-step jitter
    };

    for _ in 0..count {
        // Polar coordinates with a slight bias toward the center so the
        // composition feels anchored.
        let r = layout.f32(0.0, w.max(h) * 0.55);
        let theta = layout.f32(0.0, std::f64::consts::TAU);
        let start_x = w / 2.0 + r * theta.cos();
        let start_y = h / 2.0 + r * theta.sin();

        let segments = trail_length.max(2);
        let seg_len = layout.f32(2.4, 5.2);
        let mut x = start_x;
        let mut y = start_y;
        let alpha_start = color_stream.f32(0.55, 0.95);
        let palette = color_stream.weighted(&[
            (Color::rgba(0.98, 0.45, 0.70, 1.0), 3.0), // hot pink
            (Color::rgba(0.35, 0.85, 0.98, 1.0), 3.0), // cyan
            (Color::rgba(0.98, 0.85, 0.45, 1.0), 2.0), // gold
            (Color::rgba(0.60, 0.45, 0.98, 1.0), 2.0), // violet
        ]);
        for s in 0..segments {
            let angle = direction(x, y, &mut angle_stream);
            let nx = x + angle.cos() * seg_len;
            let ny = y + angle.sin() * seg_len;
            let fade = 1.0 - (s as f64 / segments as f64);
            let opacity = alpha_start * (0.25 + 0.75 * fade);
            ctx.surface.stroke_line(
                Vec2::new(x, y),
                Vec2::new(nx, ny),
                2.4 * fade + 0.6,
                Color {
                    a: opacity,
                    ..palette
                },
            );
            x = nx;
            y = ny;
        }
    }
    Ok(())
}

/// Build a fully validated [`Template`].
#[must_use]
pub fn build() -> Template {
    Template::new(manifest(), entry as TemplateEntry).expect("particles manifest is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid() {
        let _ = build();
    }

    #[test]
    fn determinism_across_runs() {
        use seed_canvas_adapter_svg::SvgAdapter;
        use seed_canvas_core::adapter::{Adapter, AdapterKind, AdapterRegistry};
        use seed_canvas_core::render::{render, RenderRequest};
        use seed_canvas_core::surface::OutputFormat;
        use seed_canvas_core::Seed;
        use std::sync::Arc;

        let template = build();
        let mut registry = AdapterRegistry::new();
        registry.register(Arc::new(SvgAdapter::new()) as Arc<dyn Adapter>);

        let req = || RenderRequest {
            seed: Seed::from_string("cosmos"),
            params: serde_json::json!({"count": 100, "trail_length": 5}),
            adapter: AdapterKind::Svg,
            format: OutputFormat::Json,
        };

        let a = render(&template, &req(), &registry).unwrap();
        let b = render(&template, &req(), &registry).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn count_param_is_validated() {
        let template = build();
        assert!(template
            .validate_params(serde_json::json!({"count": 5}))
            .is_err());
        assert!(template
            .validate_params(serde_json::json!({"count": 100}))
            .is_ok());
    }

    #[test]
    fn background_palette_rejects_unknown() {
        let template = build();
        assert!(template
            .validate_params(serde_json::json!({"background": "neon"}))
            .is_err());
        assert!(template
            .validate_params(serde_json::json!({"background": "paper"}))
            .is_ok());
    }
}
