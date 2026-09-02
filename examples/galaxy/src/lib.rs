//! `galaxy` example template, exposed as a library so other crates (notably
//! the seed-canvas CLI) can reuse its manifest and entry function.

use seed_canvas_core::adapter::AdapterKind;
use seed_canvas_core::surface::{Color, Vec2};
use seed_canvas_core::template::{
    Author, CanvasSize, RenderContext, Template, TemplateEntry, TemplateError, TemplateManifest,
};

/// Entry function: draws the nebula.
pub fn entry(ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
    let params = ctx.params.clone();
    let count = params
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(800) as usize;
    let core_radius = params
        .get("core_radius")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(8.0);
    let core_glow = params
        .get("core_glow")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(60.0);

    let (w, h) = (ctx.canvas.width as f64, ctx.canvas.height as f64);
    let cx = w / 2.0;
    let cy = h / 2.0;

    ctx.surface.clear(Color::rgb(0.0, 0.0, 0.0));

    let mut layout = ctx.seed.fork("layout");
    let mut color_stream = ctx.seed.fork("color");
    let mut detail = ctx.seed.fork("detail");

    for ring in (1..=6).rev() {
        let r = core_glow * (ring as f64) / 6.0;
        ctx.surface
            .fill_circle(Vec2::new(cx, cy), r, Color::rgba(0.6, 0.8, 1.0, 0.06));
    }

    for _ in 0..count {
        let r = layout.f32(0.0, w.max(h) * 0.7).sqrt() * (w.max(h)) * 0.18;
        let theta = layout.f32(0.0, std::f64::consts::TAU);
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin();
        let radius = detail.f32(0.4, 2.4);

        let palette = color_stream.weighted(&[
            (Color::rgba(1.0, 1.0, 1.0, 1.0), 6.0),
            (Color::rgba(0.6, 0.8, 1.0, 1.0), 5.0),
            (Color::rgba(0.8, 0.6, 1.0, 1.0), 2.0),
            (Color::rgba(1.0, 0.8, 0.6, 1.0), 1.0),
        ]);

        ctx.surface.fill_circle(Vec2::new(x, y), radius, palette);
    }

    ctx.surface.fill_circle(
        Vec2::new(cx, cy),
        core_radius,
        Color::rgba(1.0, 0.95, 0.85, 1.0),
    );

    Ok(())
}

/// Manifest describing this template.
#[must_use]
pub fn manifest() -> TemplateManifest {
    TemplateManifest {
        id: "galaxy".into(),
        name: "Galaxy".into(),
        version: "0.1.0".into(),
        description:
            "Deterministic particle nebula with a glowing core and configurable star count.".into(),
        authors: vec![Author {
            name: "seed-canvas contributors".into(),
            url: Some("https://github.com/seed-canvas/seed-canvas".into()),
            email: None,
        }],
        license: "MIT".into(),
        canvas: CanvasSize {
            width: 1024,
            height: 1024,
        },
        tags: vec!["particles".into(), "space".into(), "nebula".into()],
        min_seed_canvas: "0.1.0".into(),
        params_schema: serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "count": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 50_000,
                    "default": 800,
                    "description": "Number of stars in the nebula."
                },
                "core_radius": {
                    "type": "number",
                    "minimum": 0.5,
                    "maximum": 50.0,
                    "default": 8.0,
                    "description": "Radius of the bright core in pixels."
                },
                "core_glow": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 200.0,
                    "default": 60.0,
                    "description": "Outer halo size around the core."
                }
            }
        }),
        adapters: vec![AdapterKind::Server, AdapterKind::Svg],
        thumbnail: None,
    }
}

/// Build a fully validated [`Template`].
#[must_use]
pub fn build() -> Template {
    Template::new(manifest(), entry as TemplateEntry).expect("galaxy manifest is valid")
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
            params: serde_json::json!({"count": 100}),
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
            .validate_params(serde_json::json!({"count": 0}))
            .is_err());
    }
}
