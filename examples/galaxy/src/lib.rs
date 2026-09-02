//! `galaxy` example template, exposed as a library so other crates (notably
//! the seed-canvas CLI) can reuse its manifest and entry function.

use seed_canvas_core::adapter::AdapterKind;
use seed_canvas_core::surface::{Color, Vec2};
use seed_canvas_core::template::{
    Author, CanvasSize, RenderContext, Template, TemplateEntry, TemplateError, TemplateManifest,
};
use seed_canvas_core::Seed;

/// Standard normal sample via Box–Muller. `u1` is drawn from
/// `[1e-4, 1)` so the logarithm never sees zero.
fn gauss(rng: &mut Seed) -> f64 {
    let u1 = rng.f32(0.0001, 1.0);
    let u2 = rng.f32(0.0, 1.0);
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Entry function: draws a spiral galaxy.
///
/// The composition is layered back-to-front:
///
/// 1. Deep-space background (near-black blue).
/// 2. Nebula haze — large, very faint colored circles scattered along
///    the spiral arms; stacking a few hundred builds visible clouds.
/// 3. Arm stars — the bulk of the star count, positioned on logarithmic
///    spirals with Gaussian scatter that widens toward the rim.
/// 4. Disk stars — a sparse uniform field filling the disk between arms.
/// 5. Core glow — a dozen concentric layers from warm white to blue.
/// 6. Foreground bright stars — a few larger stars, each with its own
///    halo, to give the image focal points.
pub fn entry(ctx: &mut RenderContext<'_>) -> Result<(), TemplateError> {
    let params = ctx.params.clone();
    let count = params
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(2600) as usize;
    let arms = params
        .get("arms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(2)
        .max(1) as f64;
    let windings = params
        .get("windings")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.7);
    let nebula_on = params
        .get("nebula")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let (w, h) = (ctx.canvas.width as f64, ctx.canvas.height as f64);
    let cx = w / 2.0;
    let cy = h / 2.0;
    let max_r = w.min(h) * 0.46;
    let tau = std::f64::consts::TAU;

    // 1. Deep-space background — never pure black; a hint of blue reads
    //    as "night sky" on modern displays.
    ctx.surface.clear(Color::rgb(0.008, 0.009, 0.02));

    let mut nebula = ctx.seed.fork("nebula");
    let mut arm = ctx.seed.fork("arms");
    let mut disk = ctx.seed.fork("disk");
    let mut core = ctx.seed.fork("core");
    let mut bright = ctx.seed.fork("bright");

    // 2. Nebula haze — each puff is a 4-layer stack of concentric
    //    circles (bright small core, faint large rim) which reads as a
    //    radial gradient under normal alpha blending.
    if nebula_on {
        let haze_count = (count / 5).clamp(200, 700);
        for _ in 0..haze_count {
            let which = nebula.range(arms as u64) as f64;
            let t = nebula.f32(0.04, 1.0);
            let theta = which * tau / arms + t * windings * tau + gauss(&mut nebula) * 0.20;
            let r = max_r * t.powf(0.65) + gauss(&mut nebula) * max_r * 0.035;
            let x = cx + r * theta.cos();
            let y = cy + r * theta.sin() * 0.94;
            let base_radius = nebula.f32(60.0, 210.0);
            let base_alpha = nebula.f32(0.045, 0.11) * (1.0 - t * 0.4);
            let color = nebula.weighted(&[
                (Color::rgba(0.30, 0.50, 1.00, base_alpha), 5.0), // blue
                (Color::rgba(0.60, 0.40, 1.00, base_alpha), 3.0), // violet
                (Color::rgba(1.00, 0.50, 0.75, base_alpha), 2.0), // pink
                (Color::rgba(0.40, 0.90, 0.90, base_alpha), 1.0), // teal
            ]);
            for layer in (1..=4).rev() {
                let lf = layer as f64 / 4.0;
                let mut c = color;
                c.a = base_alpha * (1.0 - lf + 0.15);
                ctx.surface
                    .fill_circle(Vec2::new(x, y), base_radius * lf, c);
            }
        }
    }

    // 3. Arm stars.
    for _ in 0..count {
        let which = arm.range(arms as u64) as f64;
        let t = arm.f32(0.0, 1.0).powf(0.8);
        // Scatter widens toward the rim so the arms stay crisp near the
        // core and dissolve into the disk at the edge.
        let spread = 0.10 + 0.16 * t;
        let theta = which * tau / arms + t * windings * tau + gauss(&mut arm) * spread;
        let r = max_r * t.powf(0.65) + gauss(&mut arm) * max_r * 0.028;
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin() * 0.94;

        // Power-law sizing: many tiny stars, a handful of large ones.
        let size = 0.40 + 2.4 * arm.f32(0.0, 1.0).powf(3.2);
        let alpha = (1.0 - t * 0.45) * arm.f32(0.55, 1.0);
        // Inner stars lean warm-white, outer stars lean blue.
        let warmth = 1.0 - t;
        let color = Color::rgba(
            (0.72 + 0.28 * warmth).min(1.0),
            0.82 + 0.10 * warmth,
            1.0,
            alpha,
        );
        ctx.surface.fill_circle(Vec2::new(x, y), size, color);
    }

    // 4. Disk stars — half the arm count, spread over the whole disk so
    //    the space between arms never reads as empty.
    let disk_count = count / 2;
    for _ in 0..disk_count {
        let theta = disk.f32(0.0, tau);
        // Radial density falls off from the core.
        let r = max_r * disk.f32(0.0, 1.0).powf(0.5);
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin() * 0.94;
        let size = 0.30 + 1.3 * disk.f32(0.0, 1.0).powf(3.0);
        let alpha = disk.f32(0.15, 0.6);
        ctx.surface
            .fill_circle(Vec2::new(x, y), size, Color::rgba(0.85, 0.88, 1.0, alpha));
    }

    // 5. Core glow — a dozen stacked layers, warm at the center and
    //    cooling toward the rim.
    let layers = 12;
    for i in (1..=layers).rev() {
        let f = i as f64 / layers as f64;
        let radius = core.f32(5.0, 9.0) + max_r * 0.42 * f * f;
        let alpha = 0.035 + 0.30 * (1.0 - f).powf(1.6);
        let color = Color::rgba(0.95, 0.90 - 0.10 * f, 0.78 + 0.20 * f, alpha);
        ctx.surface.fill_circle(Vec2::new(cx, cy), radius, color);
    }
    // The brilliant center itself.
    ctx.surface.fill_circle(
        Vec2::new(cx, cy),
        core.f32(5.0, 8.0),
        Color::rgba(1.0, 0.98, 0.92, 1.0),
    );

    // 6. Foreground bright stars with individual halos.
    let bright_count = 22;
    for _ in 0..bright_count {
        let theta = bright.f32(0.0, tau);
        let r = max_r * bright.f32(0.10, 1.0).powf(0.6);
        let x = cx + r * theta.cos();
        let y = cy + r * theta.sin() * 0.94;
        let size = bright.f32(1.4, 3.2);
        let tint = bright.weighted(&[
            (Color::rgba(1.0, 1.0, 1.0, 1.0), 5.0),
            (Color::rgba(0.65, 0.78, 1.0, 1.0), 3.0), // blue giant
            (Color::rgba(1.0, 0.80, 0.60, 1.0), 3.0), // orange giant
        ]);
        // Four-layer halo behind each bright star.
        for h_i in (1..=4).rev() {
            let hf = h_i as f64 / 4.0;
            let halo_r = size * (1.0 + 5.0 * hf * hf);
            let mut halo = tint;
            halo.a = 0.05 * (1.0 - hf);
            ctx.surface.fill_circle(Vec2::new(x, y), halo_r, halo);
        }
        ctx.surface.fill_circle(Vec2::new(x, y), size, tint);
    }

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
                    "minimum": 100,
                    "maximum": 50_000,
                    "default": 2600,
                    "description": "Number of stars along the spiral arms."
                },
                "arms": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 6,
                    "default": 2,
                    "description": "Number of spiral arms."
                },
                "windings": {
                    "type": "number",
                    "minimum": 0.5,
                    "maximum": 4.0,
                    "default": 1.7,
                    "description": "How many turns each arm makes around the core."
                },
                "nebula": {
                    "type": "boolean",
                    "default": true,
                    "description": "Render the colored nebula haze along the arms."
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
            size_override: None,
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
