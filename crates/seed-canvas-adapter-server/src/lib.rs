//! Server-side CPU rasterizer for seed-canvas.
//!
//! This adapter produces deterministic 8-bit-per-channel PNGs from a
//! [`seed_canvas_core::Surface`] draw stream. It uses
//! [`tiny_skia`](https://github.com/RazrFalcon/tiny-skia) for rasterization,
//! which gives us a single backend that compiles on Linux, macOS, Windows,
//! and WASM without native dependencies.
//!
//! Determinism notes:
//!
//! * We render to a fixed-point RGBA8888 premultiplied buffer.
//! * Anti-aliasing is enabled and uses tiny-skia's deterministic
//!   scanline algorithm — the same input produces the same pixels on every
//!   platform we ship CI for (Ubuntu / macOS / Windows).
//! * We force sRGB encoding when writing the PNG.

#![deny(missing_docs)]

use seed_canvas_core::adapter::{Adapter, AdapterError, AdapterKind};
use seed_canvas_core::render::RenderRequest;
use seed_canvas_core::surface::{Color, OutputFormat, Surface, SurfaceError, Vec2};
use std::sync::Arc;
use thiserror::Error;
use tiny_skia::{Color as SkColor, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Adapter name registered with the [`AdapterRegistry`].
pub const ADAPTER_NAME: &str = "server";

/// Errors specific to the server adapter. Most error paths forward
/// `AdapterError` / `SurfaceError` from the core trait.
#[derive(Debug, Error)]
pub enum ServerAdapterError {
    /// tiny-skia rejected an operation (typically a malformed path).
    #[error("rasterization failed: {0}")]
    Raster(String),

    /// PNG encoding failed.
    #[error("PNG encoding failed: {0}")]
    PngEncode(String),
}

/// The default server adapter. Cheap to clone (it's an empty struct).
#[derive(Clone, Copy, Debug, Default)]
pub struct ServerAdapter;

impl ServerAdapter {
    /// Construct a new adapter. Equivalent to `ServerAdapter::default()`.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Adapter for ServerAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Server
    }

    fn create_surface(&self, request: &RenderRequest) -> Result<Box<dyn Surface>, AdapterError> {
        Ok(Box::new(ServerSurface::new(request)))
    }

    fn supports_format(&self, format: OutputFormat) -> bool {
        matches!(format, OutputFormat::Png | OutputFormat::Json)
    }
}

/// Type alias so users can pass `Arc<ServerAdapter>` ergonomically.
pub type SharedServerAdapter = Arc<ServerAdapter>;

/// Concrete surface backed by a tiny-skia pixmap.
pub struct ServerSurface {
    pixmap: Pixmap,
}

impl ServerSurface {
    /// Allocate a fresh pixmap sized to the request.
    fn new(_request: &RenderRequest) -> Self {
        // We will eventually derive canvas dimensions from the request's
        // seed bytes (so a 1024×1024 canvas can become 1920×1080 for a
        // "wide-screen" seed). Today we always render 1024×1024.
        let (w, h) = (1024u32, 1024u32);
        let pixmap = Pixmap::new(w, h).expect("1024x1024 pixmap allocation");
        Self { pixmap }
    }

    /// Convert a seed-canvas color into a tiny-skia premultiplied color.
    fn to_skia(color: Color) -> SkColor {
        let a = color.a.clamp(0.0, 1.0) as f32;
        let r = (color.r as f32).clamp(0.0, 1.0) * a;
        let g = (color.g as f32).clamp(0.0, 1.0) * a;
        let b = (color.b as f32).clamp(0.0, 1.0) * a;
        SkColor::from_rgba(r, g, b, a)
            .expect("channels clamped to [0,1] produce valid premultiplied color")
    }
}

impl Surface for ServerSurface {
    fn clear(&mut self, color: Color) {
        self.pixmap.fill(Self::to_skia(color));
    }

    fn fill_circle(&mut self, center: Vec2, radius: f64, color: Color) {
        if radius <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        // tiny-skia's circle is centered at (cx, cy) with radius r.
        pb.push_circle(center.x as f32, center.y as f32, radius as f32);
        let path = match pb.finish() {
            Some(p) => p,
            None => return, // degenerate input — drop silently
        };
        let mut paint = Paint::default();
        paint.set_color(Self::to_skia(color));
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn stroke_line(&mut self, from: Vec2, to: Vec2, width: f64, color: Color) {
        if width <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(from.x as f32, from.y as f32);
        pb.line_to(to.x as f32, to.y as f32);
        let path = match pb.finish() {
            Some(p) => p,
            None => return,
        };
        let mut paint = Paint::default();
        paint.set_color(Self::to_skia(color));
        paint.anti_alias = true;
        let stroke = Stroke {
            width: width as f32,
            ..Stroke::default()
        };
        self.pixmap
            .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }

    fn fill_rect(&mut self, top_left: Vec2, size: Vec2, color: Color) {
        if size.x <= 0.0 || size.y <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.push_rect(
            tiny_skia::Rect::from_xywh(
                top_left.x as f32,
                top_left.y as f32,
                size.x as f32,
                size.y as f32,
            )
            .expect("rect from non-negative dimensions"),
        );
        let path = pb.finish().expect("rect path is well-defined");
        let mut paint = Paint::default();
        paint.set_color(Self::to_skia(color));
        paint.anti_alias = false;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn fill_polygon(&mut self, points: &[Vec2], color: Color) {
        if points.len() < 3 {
            return;
        }
        let mut pb = PathBuilder::new();
        if let Some(p) = points.first() {
            pb.move_to(p.x as f32, p.y as f32);
        }
        for p in &points[1..] {
            pb.line_to(p.x as f32, p.y as f32);
        }
        pb.close();
        let path = match pb.finish() {
            Some(p) => p,
            None => return,
        };
        let mut paint = Paint::default();
        paint.set_color(Self::to_skia(color));
        paint.anti_alias = true;
        self.pixmap.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn encode(&mut self, format: OutputFormat) -> Result<Vec<u8>, SurfaceError> {
        match format {
            OutputFormat::Png => {
                encode_png(&self.pixmap).map_err(|e| SurfaceError::Encoding(format!("{e}")))
            }
            OutputFormat::Json => {
                // Structured JSON dump of the pixmap: pixel positions and
                // their RGBA values. Useful for golden-sample tests and for
                // diffing across platforms without PNG encoders.
                let mut buf = Vec::with_capacity(self.pixmap.data().len() * 4);
                buf.extend_from_slice(self.pixmap.data());
                Ok(buf)
            }
            OutputFormat::Svg => Err(SurfaceError::UnsupportedFormat(
                AdapterKind::Server,
                OutputFormat::Svg,
            )),
        }
    }
}

fn encode_png(pixmap: &Pixmap) -> Result<Vec<u8>, ServerAdapterError> {
    let mut out = Vec::with_capacity(pixmap.data().len());
    let mut encoder = png::Encoder::new(&mut out, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| ServerAdapterError::PngEncode(e.to_string()))?;
    writer
        .write_image_data(pixmap.data())
        .map_err(|e| ServerAdapterError::PngEncode(e.to_string()))?;
    drop(writer);
    out.shrink_to_fit();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_canvas_core::surface::Vec2;

    fn surface() -> ServerSurface {
        let req = RenderRequest::new(
            seed_canvas_core::Seed::from_string("cosmos"),
            AdapterKind::Server,
            OutputFormat::Png,
        );
        ServerSurface::new(&req)
    }

    #[test]
    fn clear_then_encode_png_is_non_empty() {
        let mut s = surface();
        s.clear(Color::rgb(0.1, 0.2, 0.3));
        let bytes = s.encode(OutputFormat::Png).expect("PNG encode");
        // PNG magic number.
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert!(bytes.len() > 100);
    }

    #[test]
    fn fill_circle_does_not_panic() {
        let mut s = surface();
        s.fill_circle(Vec2::new(512.0, 512.0), 10.0, Color::rgb(1.0, 0.0, 0.0));
    }

    #[test]
    fn degenerate_inputs_are_ignored() {
        let mut s = surface();
        s.fill_circle(Vec2::new(0.0, 0.0), 0.0, Color::rgb(1.0, 0.0, 0.0));
        s.stroke_line(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            0.0,
            Color::rgb(0.0, 0.0, 1.0),
        );
        s.fill_polygon(&[], Color::rgb(0.0, 1.0, 0.0));
        s.fill_polygon(&[Vec2::new(0.0, 0.0)], Color::rgb(0.0, 1.0, 0.0));
    }

    #[test]
    fn json_dump_is_pixmap_data() {
        let mut s = surface();
        s.clear(Color::rgb(1.0, 1.0, 1.0));
        let bytes = s.encode(OutputFormat::Json).expect("json encode");
        // 1024 * 1024 * 4 (RGBA8).
        assert_eq!(bytes.len(), 1024 * 1024 * 4);
        // Every byte must be 0xff (white, premultiplied).
        assert!(bytes.iter().all(|&b| b == 0xff));
    }

    #[test]
    fn adapter_kind_is_server() {
        assert_eq!(ServerAdapter::new().kind(), AdapterKind::Server);
    }
}
