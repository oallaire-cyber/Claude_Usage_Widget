//! Tray icon: colour ("tone") of a window, and the ring rasteriser.
//!
//! Tone rules (PROMPT.md Phase 3 and §4b, DECISIONS.md D-17):
//! - grey when the window is stale, reset or has no data;
//! - red at ≥ 90 % used, whatever the pace;
//! - otherwise by pace ratio: red ≥ 1.75×, amber ≥ 1.1×, else blue;
//! - when the pace guards apply (little used, window just started) or no pace can be computed, by
//!   plain thresholds instead: amber ≥ 70 %, else blue.

use serde::{Deserialize, Serialize};

use crate::merge::Status;
use crate::pace::{Pace, AHEAD_RATIO};

/// Red regardless of pace from this much used.
pub const RED_USED_PCT: f64 = 90.0;
/// Red from this pace ratio.
pub const RED_RATIO: f64 = 1.75;
/// Amber from this much used when colouring by plain thresholds.
pub const AMBER_PLAIN_PCT: f64 = 70.0;

/// Icon sizes rendered at runtime: 16 px at 100 % scaling, 20/24/32 px at 125/150/200 %.
pub const ICON_SIZES: [u32; 4] = [16, 20, 24, 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// On or under pace.
    Blue,
    /// Ahead of pace (≥ 1.1×), or ≥ 70 % when pace does not apply.
    Amber,
    /// Far ahead of pace (≥ 1.75×) or ≥ 90 % used.
    Red,
    /// Stale, reset, or no data.
    Grey,
}

pub fn tone(status: Status, used_percentage: f64, pace: Option<&Pace>) -> Tone {
    if status != Status::Ok || !used_percentage.is_finite() {
        return Tone::Grey;
    }
    if used_percentage >= RED_USED_PCT {
        return Tone::Red;
    }
    match pace.filter(|p| !p.guarded).and_then(|p| p.pace_ratio) {
        Some(r) if r >= RED_RATIO => Tone::Red,
        Some(r) if r >= AHEAD_RATIO => Tone::Amber,
        Some(_) => Tone::Blue,
        None if used_percentage >= AMBER_PLAIN_PCT => Tone::Amber,
        None => Tone::Blue,
    }
}

/// Straight (non-premultiplied) RGBA of a tone's arc.
pub fn tone_rgba(t: Tone) -> [u8; 4] {
    match t {
        Tone::Blue => [0x3D, 0x9B, 0xFF, 0xFF],
        Tone::Amber => [0xF5, 0xA5, 0x24, 0xFF],
        Tone::Red => [0xE8, 0x48, 0x4D, 0xFF],
        Tone::Grey => [0x9A, 0x9A, 0x9A, 0xFF],
    }
}

/// The unfilled part of the ring: a neutral grey, half transparent, visible on light and dark taskbars.
pub const TRACK_RGBA: [u8; 4] = [0x80, 0x80, 0x80, 0x73];

/// Samples per pixel side for anti-aliasing (4 × 4 = 16 samples per pixel).
const SUPERSAMPLE: u32 = 4;

/// Render the tray ring: a full circle track, with an arc from 12 o'clock clockwise covering
/// `fraction` (0..=1) of it in the tone's colour. Returns `size × size` RGBA bytes, row-major.
pub fn render_ring(size: u32, fraction: f64, t: Tone) -> Vec<u8> {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let s = size as f64;
    let c = s / 2.0;
    let outer = c - 0.5;
    let thickness = (s * 0.2).max(3.0);
    let inner = outer - thickness;
    let arc = tone_rgba(t);
    let sweep = fraction * std::f64::consts::TAU;
    let n = SUPERSAMPLE * SUPERSAMPLE;

    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            // Premultiplied accumulation.
            let mut acc = [0f64; 4];
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = px as f64 + (sx as f64 + 0.5) / SUPERSAMPLE as f64 - c;
                    let y = py as f64 + (sy as f64 + 0.5) / SUPERSAMPLE as f64 - c;
                    let d = (x * x + y * y).sqrt();
                    if d < inner || d > outer {
                        continue;
                    }
                    // Angle from 12 o'clock, clockwise (screen y points down).
                    let mut a = x.atan2(-y);
                    if a < 0.0 {
                        a += std::f64::consts::TAU;
                    }
                    let col = if a < sweep { arc } else { TRACK_RGBA };
                    let alpha = col[3] as f64 / 255.0;
                    for (i, v) in acc.iter_mut().take(3).enumerate() {
                        *v += col[i] as f64 * alpha;
                    }
                    acc[3] += alpha;
                }
            }
            let a = acc[3] / n as f64;
            if a <= 0.0 {
                continue;
            }
            let o = ((py * size + px) * 4) as usize;
            for i in 0..3 {
                out[o + i] = (acc[i] / acc[3]).round().clamp(0.0, 255.0) as u8;
            }
            out[o + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// The icon size for a display scale factor (1.0 = 100 %): the smallest standard size covering
/// 16 px × scale.
pub fn icon_size_for_scale(scale: f64) -> u32 {
    let want = if scale.is_finite() && scale > 0.0 {
        16.0 * scale
    } else {
        16.0
    };
    ICON_SIZES
        .iter()
        .copied()
        .find(|s| *s as f64 >= want - 0.01)
        .unwrap_or(32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pace::{PaceStatus, ProjectionBasis};

    fn pace(ratio: f64, guarded: bool) -> Pace {
        Pace {
            elapsed_fraction: 0.5,
            pace_ratio: Some(ratio),
            status: PaceStatus::OnTrack,
            projected_exhaustion_at: None,
            basis: ProjectionBasis::Average,
            guarded,
        }
    }

    #[test]
    fn tone_by_pace_ratio() {
        let ok = Status::Ok;
        assert_eq!(tone(ok, 40.0, Some(&pace(0.8, false))), Tone::Blue);
        assert_eq!(tone(ok, 40.0, Some(&pace(1.0999, false))), Tone::Blue);
        assert_eq!(tone(ok, 40.0, Some(&pace(1.1, false))), Tone::Amber);
        assert_eq!(tone(ok, 40.0, Some(&pace(1.7499, false))), Tone::Amber);
        assert_eq!(tone(ok, 40.0, Some(&pace(1.75, false))), Tone::Red);
    }

    #[test]
    fn red_from_90_percent_regardless_of_pace() {
        let ok = Status::Ok;
        assert_eq!(tone(ok, 89.99, Some(&pace(0.95, false))), Tone::Blue);
        assert_eq!(tone(ok, 90.0, Some(&pace(0.95, false))), Tone::Red);
        assert_eq!(tone(ok, 95.0, None), Tone::Red);
    }

    #[test]
    fn guards_fall_back_to_plain_thresholds() {
        let ok = Status::Ok;
        // Guarded: a huge ratio early in the window does not turn the icon red.
        assert_eq!(tone(ok, 15.0, Some(&pace(3.0, true))), Tone::Blue);
        assert_eq!(tone(ok, 69.9, Some(&pace(5.0, true))), Tone::Blue);
        assert_eq!(tone(ok, 70.0, Some(&pace(5.0, true))), Tone::Amber);
        // No pace at all (Desktop-only data): plain thresholds.
        assert_eq!(tone(ok, 50.0, None), Tone::Blue);
        assert_eq!(tone(ok, 72.0, None), Tone::Amber);
    }

    #[test]
    fn grey_unless_ok() {
        for st in [Status::Stale, Status::WindowReset, Status::NoData] {
            assert_eq!(tone(st, 95.0, Some(&pace(2.0, false))), Tone::Grey);
        }
        assert_eq!(tone(Status::Ok, f64::NAN, None), Tone::Grey);
    }

    fn px(img: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * size + x) * 4) as usize;
        [img[o], img[o + 1], img[o + 2], img[o + 3]]
    }

    /// Pixel test: for each tone, a half-filled ring has the tone's exact colour at 3 o'clock (inside
    /// the arc), the track colour at 9 o'clock (outside it), and transparent centre and corners.
    #[test]
    fn pixels_match_tone_colours() {
        for size in ICON_SIZES {
            let c = size / 2;
            let mid = size / 2 - 1 - (size as f64 * 0.1) as u32; // radial middle of the ring
            for t in [Tone::Blue, Tone::Amber, Tone::Red, Tone::Grey] {
                let img = render_ring(size, 0.5, t);
                assert_eq!(img.len(), (size * size * 4) as usize);
                assert_eq!(
                    px(&img, size, c + mid, c),
                    tone_rgba(t),
                    "{size}px {t:?} 3 o'clock"
                );
                assert_eq!(
                    px(&img, size, c - 1 - mid, c),
                    TRACK_RGBA,
                    "{size}px 9 o'clock"
                );
                assert_eq!(px(&img, size, c, c)[3], 0, "{size}px centre transparent");
                assert_eq!(px(&img, size, 0, 0)[3], 0, "{size}px corner transparent");
            }
        }
    }

    /// Pixel test tied to the thresholds: the colour actually drawn changes exactly at the boundaries.
    #[test]
    fn pixels_follow_thresholds() {
        let size = 32;
        let at_3 = |used: f64, p: Option<Pace>| {
            let img = render_ring(size, used / 100.0, tone(Status::Ok, used, p.as_ref()));
            px(&img, size, 16 + 13, 16)
        };
        assert_eq!(at_3(50.0, Some(pace(1.09, false))), tone_rgba(Tone::Blue));
        assert_eq!(at_3(50.0, Some(pace(1.10, false))), tone_rgba(Tone::Amber));
        assert_eq!(at_3(50.0, Some(pace(1.75, false))), tone_rgba(Tone::Red));
        assert_eq!(at_3(89.0, Some(pace(1.0, false))), tone_rgba(Tone::Blue));
        assert_eq!(at_3(90.0, Some(pace(1.0, false))), tone_rgba(Tone::Red));
        assert_eq!(at_3(70.0, None), tone_rgba(Tone::Amber));
        let stale = render_ring(size, 0.5, tone(Status::Stale, 50.0, None));
        assert_eq!(px(&stale, size, 29, 16), tone_rgba(Tone::Grey));
    }

    #[test]
    fn fill_fraction() {
        let size = 32;
        // Empty: 3 o'clock is track. Full: 9 o'clock is arc.
        assert_eq!(
            px(&render_ring(size, 0.0, Tone::Blue), size, 29, 16),
            TRACK_RGBA
        );
        assert_eq!(
            px(&render_ring(size, 1.0, Tone::Blue), size, 2, 16),
            tone_rgba(Tone::Blue)
        );
        // Out-of-range fractions are clamped, never panic.
        assert_eq!(
            render_ring(size, 1.7, Tone::Red),
            render_ring(size, 1.0, Tone::Red)
        );
        assert_eq!(
            render_ring(size, f64::NAN, Tone::Red),
            render_ring(size, 0.0, Tone::Red)
        );
    }

    #[test]
    fn sizes_for_scale() {
        assert_eq!(icon_size_for_scale(1.0), 16);
        assert_eq!(icon_size_for_scale(1.25), 20);
        assert_eq!(icon_size_for_scale(1.5), 24);
        assert_eq!(icon_size_for_scale(1.75), 32);
        assert_eq!(icon_size_for_scale(2.0), 32);
        assert_eq!(icon_size_for_scale(3.0), 32);
        assert_eq!(icon_size_for_scale(0.0), 16);
    }
}
