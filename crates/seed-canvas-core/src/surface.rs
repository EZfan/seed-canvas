//! Abstract surface that adapters translate into PNG pixels, SVG nodes, etc.
//!
//! The surface API is intentionally minimal — it covers the primitives that
//! the official templates use, and nothing more. Adding more primitives is
//! cheap; keeping the surface small keeps adapters maintainable and template
//! portability high.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 2D point or vector in canvas coordinates. Origin is top-left; +x is right;
/// +y is down. This matches every graphics backend we target (PNG, SVG, WebGL,
/// WebGPU).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Vec2 {
    /// Construct a new vector.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Polar conversion: angle in radians, radius in pixels.
    #[must_use]
    pub fn from_polar(angle: f64, radius: f64) -> Self {
        Self {
            x: radius * angle.cos(),
            y: radius * angle.sin(),
        }
    }
}

/// sRGB color with optional alpha (defaults to opaque). Components are
/// normalized to `[0, 1]`. Adapters convert to their native format
/// (8-bit PNG, hex SVG, etc.) using sRGB-aware gamma encoding.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    /// Red channel, `[0, 1]`.
    pub r: f64,
    /// Green channel, `[0, 1]`.
    pub g: f64,
    /// Blue channel, `[0, 1]`.
    pub b: f64,
    /// Alpha channel, `[0, 1]`. Defaults to `1.0` (opaque).
    #[serde(default = "Color::default_alpha")]
    pub a: f64,
}

impl Color {
    /// Construct an opaque RGB color.
    #[must_use]
    pub const fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Construct an RGBA color.
    #[must_use]
    pub const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// Construct a color from an 8-bit-per-channel integer triple.
    #[must_use]
    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: 1.0,
        }
    }

    /// Convenience: linear interpolation in sRGB space.
    ///
    /// `t == 0` returns `self`; `t == 1` returns `other`. Values outside
    /// `[0, 1]` are extrapolated linearly.
    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    fn default_alpha() -> f64 {
        1.0
    }
}

/// Named-color shortcuts. Templates can use these instead of building raw
/// [`Color`] values, and the gallery can render them as readable labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamedColor {
    /// Pure black `#000`.
    Black,
    /// Pure white `#fff`.
    White,
    /// Pure red `#f00`.
    Red,
    /// Pure green `#0f0`.
    Green,
    /// Pure blue `#00f`.
    Blue,
    /// Fully transparent.
    Transparent,
}

impl NamedColor {
    /// Resolve to a concrete [`Color`].
    #[must_use]
    pub const fn resolve(self) -> Color {
        match self {
            Self::Black => Color::rgb(0.0, 0.0, 0.0),
            Self::White => Color::rgb(1.0, 1.0, 1.0),
            Self::Red => Color::rgb(1.0, 0.0, 0.0),
            Self::Green => Color::rgb(0.0, 1.0, 0.0),
            Self::Blue => Color::rgb(0.0, 0.0, 1.0),
            Self::Transparent => Color::rgba(0.0, 0.0, 0.0, 0.0),
        }
    }
}

/// Re-export as a constant table for ergonomic use.
#[allow(non_upper_case_globals)]
pub const NAMED_COLORS: &[(NamedColor, &str)] = &[
    (NamedColor::Black, "#000000"),
    (NamedColor::White, "#ffffff"),
    (NamedColor::Red, "#ff0000"),
    (NamedColor::Green, "#00ff00"),
    (NamedColor::Blue, "#0000ff"),
    (NamedColor::Transparent, "transparent"),
];

/// Abstract surface every adapter implements.
///
/// Templates write through this interface and remain backend-agnostic.
/// All methods take their inputs by value to make the contract obvious in
/// template source code.
pub trait Surface {
    /// Clear the entire canvas to `color`. Called exactly once at the start
    /// of every render; adapters may treat this as a hint to allocate their
    /// pixel buffer.
    fn clear(&mut self, color: Color);

    /// Draw a filled circle at `center` with the given `radius` (pixels).
    fn fill_circle(&mut self, center: Vec2, radius: f64, color: Color);

    /// Draw a stroked line segment from `from` to `to` with the given pixel
    /// `width`. Lines shorter than one pixel are still drawn (adapters may
    /// antialias or skip).
    fn stroke_line(&mut self, from: Vec2, to: Vec2, width: f64, color: Color);

    /// Draw a filled axis-aligned rectangle with top-left at `top_left` and
    /// dimensions `size`.
    fn fill_rect(&mut self, top_left: Vec2, size: Vec2, color: Color);

    /// Draw a filled polygon with the given vertices (in order). Adapters
    /// may auto-close or anti-alias the edge.
    fn fill_polygon(&mut self, points: &[Vec2], color: Color);

    /// Finalize the surface and produce the requested artifact.
    ///
    /// This is the **only** allocation-heavy call in the render pipeline;
    /// everything before it is in-place.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::UnsupportedFormat`] if the adapter cannot
    /// produce the requested format, or an adapter-specific error (e.g. PNG
    /// encoding failure).
    fn encode(&mut self, format: OutputFormat) -> Result<Vec<u8>, SurfaceError>;
}

/// Output formats an adapter may be asked to produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// 8-bit-per-channel RGBA PNG.
    Png,
    /// Well-formed SVG 1.1 document.
    Svg,
    /// Structured JSON describing the draw call stream (debugging /
    ///   golden-sample testing).
    Json,
}

impl OutputFormat {
    /// Canonical file extension, including leading dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => ".png",
            Self::Svg => ".svg",
            Self::Json => ".json",
        }
    }

    /// MIME type, suitable for HTTP `Content-Type` headers.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Svg => "image/svg+xml",
            Self::Json => "application/json",
        }
    }
}

/// Errors raised by [`Surface::encode`] and friends.
#[derive(Debug, Error)]
pub enum SurfaceError {
    /// Adapter was asked for a format it does not implement.
    #[error("adapter {0:?} cannot produce format {1:?}")]
    UnsupportedFormat(AdapterKind, OutputFormat),

    /// Underlying encoder (PNG / SVG / etc.) failed.
    #[error("encoding failed: {0}")]
    Encoding(String),

    /// Surface received malformed input (e.g. a polygon with fewer than
    /// three vertices).
    #[error("invalid input: {0}")]
    Invalid(String),
}

/// Re-exported here so adapters can refer to it without taking a second
/// dependency on the adapter module.
pub use crate::adapter::AdapterKind;
