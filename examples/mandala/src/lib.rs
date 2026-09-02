//! `mandala` example template — a symmetric mandala drawn with N-fold
//! rotational symmetry. Renders concentric rings of petal-like polygons.

use seed_canvas_core::adapter::AdapterKind;
use seed_canvas_core::surface::{Color, Vec2};
use seed_canvas_core::template::{
    Author, CanvasSize, RenderContext, Template, TemplateEntry, TemplateError, TemplateManifest,
};

/// Manifest describing this template.
#[must_use]
pub fn manifest() -> TemplateManifest {
    TemplateManifest {
        id: "mandala".into(),
        name: "Mandala".into(),
        version: "0.1.0".into(),
        description:
            "Symmetric mandala with N-fold rotational symmetry and concentric petal rings.".into(),
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
        tags: vec!["mandala".into(), "symmetry".into(), "geometric".into()],
        min_seed_canvas: "0.1.0".into(),
        params_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "symmetry": {
                    "type": "integer",
                    "minimum": 4,
                    "maximum": 36,
                    "default": 12,
                    "description": "Rotational symmetry (N-fold)."
                },
                "rings": {
                    "type": "integer",
                    "minimum": 2,
                    "maximum": 32,
                    "default": 8,
                    "description": "Number of concentric rings."
                },
                "background": {
                    "type": "string",
                    "enum": ["midnight", "ivory", "rose"],
                    "default": "midnight",
                    "description": "Background palette."
                }
            }
        }),
        adapters: vec![AdapterKind::Server, AdapterKind::Svg],
        thumbnail: None,
    }
}

/// Entry function: draws the mandala.
pub fn entry(ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
    let params = ctx.params.clone();
    let symmetry = params
        .get("symmetry")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(12) as usize;
    let rings = params
        .get("rings")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(8) as usize;
    let background = params
        .get("background")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("midnight");

    let (w, h) = (ctx.canvas.width as f64, ctx.canvas.height as f64);
    let cx = w / 2.0;
    let cy = h / 2.0;

    let bg = match background {
        "ivory" => Color::rgb(0.97, 0.94, 0.86),
        "rose" => Color::rgb(0.98, 0.92, 0.92),
        _ => Color::rgb(0.04, 0.04, 0.10),
    };
    let fg = match background {
        "ivory" => Color::rgb(0.20, 0.10, 0.05),
        "rose" => Color::rgb(0.50, 0.10, 0.30),
        _ => Color::rgba(0.95, 0.85, 0.65, 1.0),
    };
    let accent = match background {
        "ivory" => Color::rgb(0.45, 0.20, 0.10),
        "rose" => Color::rgb(0.80, 0.30, 0.55),
        _ => Color::rgba(0.45, 0.85, 0.95, 1.0),
    };

    ctx.surface.clear(bg);

    let mut ring_stream = ctx.seed.fork("rings");
    let mut color_stream = ctx.seed.fork("color");
    let tau = std::f64::consts::TAU;
    let max_r = w.min(h) * 0.46;

    // A petal is a leaf-shaped hexagon anchored at inner radius `r0` and
    // pointing outward to `r1`. `width` scales the bulge of the two side
    // vertex pairs; 0.0 would degenerate to a radial line.
    let petal_points = |theta: f64, r0: f64, r1: f64, width: f64| -> Vec<Vec2> {
        let dx = theta.cos();
        let dy = theta.sin();
        let px = -dy;
        let py = dx;
        let w = width * (r1 - r0);
        let at = |r: f64, side: f64, bulge: f64| {
            Vec2::new(
                cx + r * dx + side * bulge * w * px,
                cy + r * dy + side * bulge * w * py,
            )
        };
        let mid_r = r0 + (r1 - r0) * 0.45;
        let shoulder_r = r0 + (r1 - r0) * 0.82;
        vec![
            at(r0, 0.0, 0.0),
            at(mid_r, 1.0, 1.0),
            at(shoulder_r, 0.55, 1.0),
            at(r1, 0.0, 0.0),
            at(shoulder_r, -0.55, 1.0),
            at(mid_r, -1.0, 1.0),
        ]
    };

    for r in 0..rings {
        // Rings expand outward; keep a small inner hole around the core.
        let r1 = max_r * ((r + 1) as f64 / rings as f64).powf(0.85);
        let r0 = if r == 0 {
            max_r * 0.03
        } else {
            max_r * ((r as f64) / rings as f64).powf(0.85) * 0.96
        };
        let ring_color = if r % 2 == 0 { fg } else { accent };
        let width = ring_stream.f32(0.30, 0.62);
        let alpha = ring_stream.f32(0.62, 0.88);

        // Faint guide circle for each ring.
        let seg = 96;
        for i in 0..seg {
            let a0 = tau * (i as f64) / (seg as f64);
            let a1 = tau * ((i + 1) as f64) / (seg as f64);
            ctx.surface.stroke_line(
                Vec2::new(cx + r1 * a0.cos(), cy + r1 * a0.sin()),
                Vec2::new(cx + r1 * a1.cos(), cy + r1 * a1.sin()),
                1.0,
                Color {
                    a: alpha * 0.30,
                    ..ring_color
                },
            );
        }

        for s in 0..symmetry {
            let theta = (s as f64) * tau / (symmetry as f64) + ring_stream.f32(-0.015, 0.015);
            let tint = if color_stream.branch(0.35) {
                accent
            } else {
                ring_color
            };
            let pts = petal_points(theta, r0, r1, width);
            ctx.surface.fill_polygon(&pts, Color { a: alpha, ..tint });
        }
    }

    // Central core circle.
    ctx.surface
        .fill_circle(Vec2::new(cx, cy), w.min(h) * 0.04, accent);
    ctx.surface
        .fill_circle(Vec2::new(cx, cy), w.min(h) * 0.02, fg);
    Ok(())
}

/// Build a fully validated [`Template`].
#[must_use]
pub fn build() -> Template {
    Template::new(manifest(), entry as TemplateEntry).expect("mandala manifest is valid")
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
            params: serde_json::json!({"symmetry": 8, "rings": 3}),
            adapter: AdapterKind::Svg,
            format: OutputFormat::Json,
            size_override: None,
        };

        let a = render(&template, &req(), &registry).unwrap();
        let b = render(&template, &req(), &registry).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn symmetry_must_be_at_least_4() {
        let template = build();
        assert!(template
            .validate_params(serde_json::json!({"symmetry": 3}))
            .is_err());
        assert!(template
            .validate_params(serde_json::json!({"symmetry": 4}))
            .is_ok());
    }
}
