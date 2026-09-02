//! SVG vector adapter for seed-canvas.
//!
//! Instead of rasterizing, this adapter accumulates every surface call into
//! a flat list of `<circle>`, `<line>`, `<rect>`, and `<polygon>` elements
//! and emits a self-contained, deterministic SVG 1.1 document on
//! [`Surface::encode`].
//!
//! Determinism comes from emitting every coordinate as a 4-decimal-place
//! fixed-point number. SVG is a vector format, so there is no anti-aliasing
//! or rasterization drift to worry about — the same template + seed
//! always produces byte-identical SVG across platforms and browsers.

#![deny(missing_docs)]

use seed_canvas_core::adapter::{Adapter, AdapterError, AdapterKind};
use seed_canvas_core::render::RenderRequest;
use seed_canvas_core::surface::{Color, OutputFormat, Surface, SurfaceError, Vec2};
use std::fmt::Write as _;
use thiserror::Error;

/// Adapter name registered with the [`AdapterRegistry`].
pub const ADAPTER_NAME: &str = "svg";

/// Errors specific to the SVG adapter.
#[derive(Debug, Error)]
pub enum SvgAdapterError {
    /// Buffer formatting overflowed.
    #[error("formatting failed: {0}")]
    Fmt(#[from] std::fmt::Error),
}

/// Default SVG adapter. Cheap to clone.
#[derive(Clone, Copy, Debug, Default)]
pub struct SvgAdapter;

impl SvgAdapter {
    /// Construct a new adapter. Equivalent to `SvgAdapter::default()`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Adapter for SvgAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Svg
    }

    fn create_surface(&self, request: &RenderRequest) -> Result<Box<dyn Surface>, AdapterError> {
        // Default canvas: 1024x1024, override later when templates pass
        // explicit dimensions through their manifests.
        Ok(Box::new(SvgSurface::new(1024, 1024, request)))
    }

    fn supports_format(&self, format: OutputFormat) -> bool {
        matches!(format, OutputFormat::Svg | OutputFormat::Json)
    }
}

/// Concrete surface that buffers every draw call as an SVG element.
pub struct SvgSurface {
    width: u32,
    height: u32,
    background: Option<Color>,
    elements: Vec<Element>,
    /// Optional per-element tag, used by `Json` output to keep deterministic
    /// ordering even after deterministic stream IDs are assigned.
    next_id: u32,
}

impl SvgSurface {
    /// Allocate a fresh surface with the given dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32, _request: &RenderRequest) -> Self {
        Self {
            width,
            height,
            background: None,
            elements: Vec::new(),
            next_id: 0,
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn srgb(c: Color) -> String {
        // sRGB encoding in [0, 255] hex.
        let r = ((c.r.clamp(0.0, 1.0)) * 255.0).round() as u8;
        let g = ((c.g.clamp(0.0, 1.0)) * 255.0).round() as u8;
        let b = ((c.b.clamp(0.0, 1.0)) * 255.0).round() as u8;
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

#[derive(Clone, Debug)]
enum Element {
    Circle {
        id: u32,
        cx: f32,
        cy: f32,
        r: f32,
        fill: String,
        opacity: f32,
    },
    Line {
        id: u32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke: String,
        stroke_width: f32,
        opacity: f32,
    },
    Rect {
        id: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: String,
        opacity: f32,
    },
    Polygon {
        id: u32,
        points: Vec<(f32, f32)>,
        fill: String,
        opacity: f32,
    },
}

impl Element {
    fn to_svg(&self, out: &mut String) -> std::fmt::Result {
        match *self {
            Element::Circle {
                id,
                cx,
                cy,
                r,
                ref fill,
                opacity,
            } => write!(
                out,
                r#"<circle id="e{id}" cx="{cx:.4}" cy="{cy:.4}" r="{r:.4}" fill="{fill}" opacity="{opacity:.4}"/>"#,
            ),
            Element::Line {
                id,
                x1,
                y1,
                x2,
                y2,
                ref stroke,
                stroke_width,
                opacity,
            } => write!(
                out,
                r#"<line id="e{id}" x1="{x1:.4}" y1="{y1:.4}" x2="{x2:.4}" y2="{y2:.4}" stroke="{stroke}" stroke-width="{stroke_width:.4}" opacity="{opacity:.4}"/>"#,
            ),
            Element::Rect {
                id,
                x,
                y,
                w,
                h,
                ref fill,
                opacity,
            } => write!(
                out,
                r#"<rect id="e{id}" x="{x:.4}" y="{y:.4}" width="{w:.4}" height="{h:.4}" fill="{fill}" opacity="{opacity:.4}"/>"#,
            ),
            Element::Polygon {
                id,
                ref points,
                ref fill,
                opacity,
            } => {
                write!(out, r#"<polygon id="e{id}" points=""#)?;
                for (i, (x, y)) in points.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    write!(out, "{x:.4},{y:.4}")?;
                }
                write!(out, r#"" fill="{fill}" opacity="{opacity:.4}"/>"#)
            }
        }
    }
}

impl Surface for SvgSurface {
    fn clear(&mut self, color: Color) {
        self.background = Some(color);
        // `clear` replaces the canvas; older elements are discarded so a
        // template can stage complex compositing flows.
        self.elements.clear();
    }

    fn fill_circle(&mut self, center: Vec2, radius: f64, color: Color) {
        if radius <= 0.0 {
            return;
        }
        let id = self.next_id();
        self.elements.push(Element::Circle {
            id,
            cx: center.x as f32,
            cy: center.y as f32,
            r: radius as f32,
            fill: Self::srgb(color),
            opacity: color.a.clamp(0.0, 1.0) as f32,
        });
    }

    fn stroke_line(&mut self, from: Vec2, to: Vec2, width: f64, color: Color) {
        if width <= 0.0 {
            return;
        }
        let id = self.next_id();
        self.elements.push(Element::Line {
            id,
            x1: from.x as f32,
            y1: from.y as f32,
            x2: to.x as f32,
            y2: to.y as f32,
            stroke: Self::srgb(color),
            stroke_width: width as f32,
            opacity: color.a.clamp(0.0, 1.0) as f32,
        });
    }

    fn fill_rect(&mut self, top_left: Vec2, size: Vec2, color: Color) {
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let id = self.next_id();
        self.elements.push(Element::Rect {
            id,
            x: top_left.x as f32,
            y: top_left.y as f32,
            w: size.x as f32,
            h: size.y as f32,
            fill: Self::srgb(color),
            opacity: color.a.clamp(0.0, 1.0) as f32,
        });
    }

    fn fill_polygon(&mut self, points: &[Vec2], color: Color) {
        if points.len() < 3 {
            return;
        }
        let id = self.next_id();
        let pts: Vec<(f32, f32)> = points.iter().map(|p| (p.x as f32, p.y as f32)).collect();
        self.elements.push(Element::Polygon {
            id,
            points: pts,
            fill: Self::srgb(color),
            opacity: color.a.clamp(0.0, 1.0) as f32,
        });
    }

    fn encode(&mut self, format: OutputFormat) -> Result<Vec<u8>, SurfaceError> {
        match format {
            OutputFormat::Svg => Ok(self.to_svg_document().into_bytes()),
            OutputFormat::Json => {
                let body = serde_json::to_vec(&ElementList {
                    width: self.width,
                    height: self.height,
                    background: self.background.map(Self::srgb),
                    elements: self
                        .elements
                        .iter()
                        .map(JsonElement::from)
                        .collect::<Vec<_>>(),
                })
                .map_err(|e| SurfaceError::Encoding(format!("json: {e}")))?;
                Ok(body)
            }
            _ => Err(SurfaceError::UnsupportedFormat(AdapterKind::Svg, format)),
        }
    }
}

impl SvgSurface {
    fn to_svg_document(&self) -> String {
        let mut out = String::with_capacity(64 + self.elements.len() * 96);
        let _ = writeln!(out, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        let _ = writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#,
            w = self.width,
            h = self.height,
        );
        if let Some(bg) = self.background {
            let _ = writeln!(
                out,
                r#"<rect id="bg" x="0" y="0" width="{w}" height="{h}" fill="{fill}"/>"#,
                w = self.width,
                h = self.height,
                fill = Self::srgb(bg),
            );
        }
        for el in &self.elements {
            let _ = el.to_svg(&mut out);
        }
        out.push_str("</svg>\n");
        out
    }
}

#[derive(serde::Serialize)]
struct ElementList {
    width: u32,
    height: u32,
    background: Option<String>,
    elements: Vec<JsonElement>,
}

#[derive(serde::Serialize)]
struct JsonElement {
    id: u32,
    kind: &'static str,
    fill: String,
    opacity: f32,
    #[serde(flatten)]
    body: serde_json::Value,
}

impl From<&Element> for JsonElement {
    fn from(el: &Element) -> Self {
        match *el {
            Element::Circle {
                id,
                cx,
                cy,
                r,
                ref fill,
                opacity,
            } => Self {
                id,
                kind: "circle",
                fill: fill.clone(),
                opacity,
                body: serde_json::json!({"cx": cx, "cy": cy, "r": r}),
            },
            Element::Line {
                id,
                x1,
                y1,
                x2,
                y2,
                ref stroke,
                stroke_width,
                opacity,
            } => Self {
                id,
                kind: "line",
                fill: stroke.clone(),
                opacity,
                body: serde_json::json!({"x1": x1, "y1": y1, "x2": x2, "y2": y2, "stroke_width": stroke_width}),
            },
            Element::Rect {
                id,
                x,
                y,
                w,
                h,
                ref fill,
                opacity,
            } => Self {
                id,
                kind: "rect",
                fill: fill.clone(),
                opacity,
                body: serde_json::json!({"x": x, "y": y, "w": w, "h": h}),
            },
            Element::Polygon {
                id,
                ref points,
                ref fill,
                opacity,
            } => Self {
                id,
                kind: "polygon",
                fill: fill.clone(),
                opacity,
                body: serde_json::json!({"points": points.iter().map(|(x, y)| serde_json::json!([x, y])).collect::<Vec<_>>()}),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface() -> SvgSurface {
        let req = RenderRequest::new(
            seed_canvas_core::Seed::from_string("cosmos"),
            AdapterKind::Svg,
            OutputFormat::Svg,
        );
        SvgSurface::new(640, 480, &req)
    }

    #[test]
    fn empty_surface_produces_valid_svg() {
        let mut s = surface();
        let bytes = s.encode(OutputFormat::Svg).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.starts_with("<?xml"));
        assert!(s.contains("<svg"));
        assert!(s.contains("</svg>"));
        assert!(s.contains("viewBox=\"0 0 640 480\""));
    }

    #[test]
    fn clear_emits_background_rect() {
        let mut s = surface();
        s.clear(Color::rgb(0.1, 0.2, 0.3));
        let bytes = s.encode(OutputFormat::Svg).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        // 0.1 * 255 = 25.5 -> 26 (#1a), 0.2 * 255 = 51 (#33), 0.3 * 255 = 76.5 -> 77 (#4d)
        assert!(s.contains(r##"fill="#1a334d""##));
        assert!(s.contains(r##"<rect id="bg""##));
    }

    #[test]
    fn all_draw_calls_produce_xml() {
        let mut s = surface();
        s.clear(Color::rgb(0.0, 0.0, 0.0));
        s.fill_circle(Vec2::new(10.0, 20.0), 5.0, Color::rgb(1.0, 0.0, 0.0));
        s.stroke_line(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 100.0),
            1.0,
            Color::rgb(1.0, 1.0, 1.0),
        );
        s.fill_rect(
            Vec2::new(5.0, 5.0),
            Vec2::new(10.0, 10.0),
            Color::rgb(0.0, 1.0, 0.0),
        );
        s.fill_polygon(
            &[
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(5.0, 10.0),
            ],
            Color::rgb(0.0, 0.0, 1.0),
        );
        let bytes = s.encode(OutputFormat::Svg).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.contains("<circle"));
        assert!(s.contains("<line"));
        assert!(s.contains("<rect"));
        assert!(s.contains("<polygon"));
    }

    #[test]
    fn svg_is_deterministic() {
        let draw = |mut s: SvgSurface| {
            s.clear(Color::rgb(0.0, 0.0, 0.0));
            for i in 0..10 {
                let x = (i as f32) * 5.0;
                s.fill_circle(Vec2::new(x as f64, 50.0), 2.0, Color::rgb(1.0, 1.0, 1.0));
            }
            s.encode(OutputFormat::Svg).unwrap()
        };
        let a = draw(surface());
        let b = draw(surface());
        assert_eq!(a, b);
    }

    #[test]
    fn json_output_is_well_formed() {
        let mut s = surface();
        s.fill_circle(Vec2::new(0.0, 0.0), 1.0, Color::rgb(1.0, 0.0, 0.0));
        let bytes = s.encode(OutputFormat::Json).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("elements").is_some());
    }
}
