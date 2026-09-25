//! Where the popup goes: next to the tray icon, on the side of the screen the taskbar is on, inside
//! the monitor's work area. Pure geometry in physical pixels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn centre(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// Which edge the taskbar is on: the side where the work area is smaller than the monitor. With an
/// auto-hiding taskbar (work area = monitor), the monitor edge nearest the anchor.
pub fn taskbar_edge(monitor: Rect, work: Rect, anchor: Rect) -> Edge {
    if work.bottom() < monitor.bottom() {
        Edge::Bottom
    } else if work.y > monitor.y {
        Edge::Top
    } else if work.x > monitor.x {
        Edge::Left
    } else if work.right() < monitor.right() {
        Edge::Right
    } else {
        let (cx, cy) = anchor.centre();
        let d = [
            (monitor.bottom() - cy, Edge::Bottom),
            (cy - monitor.y, Edge::Top),
            (cx - monitor.x, Edge::Left),
            (monitor.right() - cx, Edge::Right),
        ];
        d.iter()
            .min_by_key(|(dist, _)| *dist)
            .map(|(_, e)| *e)
            .unwrap()
    }
}

fn clamp_span(start: i32, len: i32, lo: i32, hi: i32) -> i32 {
    // If the popup is larger than the space, pin it to the start.
    start.min(hi - len).max(lo)
}

/// Top-left corner for a popup of `size` (w, h), `margin` px away from the taskbar, centred on the
/// anchor along the taskbar and kept inside the work area.
pub fn popup_position(
    monitor: Rect,
    work: Rect,
    anchor: Rect,
    size: (i32, i32),
    margin: i32,
) -> (i32, i32) {
    let (w, h) = size;
    let (cx, cy) = anchor.centre();
    let x_along = clamp_span(cx - w / 2, w, work.x + margin, work.right() - margin);
    let y_along = clamp_span(cy - h / 2, h, work.y + margin, work.bottom() - margin);
    match taskbar_edge(monitor, work, anchor) {
        Edge::Bottom => (x_along, (work.bottom() - h - margin).max(work.y)),
        Edge::Top => (x_along, work.y + margin),
        Edge::Left => (work.x + margin, y_along),
        Edge::Right => ((work.right() - w - margin).max(work.x), y_along),
    }
}

/// Whether a saved window position is still usable: its centre lies in some monitor's work area.
pub fn visible_on(pos: (i32, i32), size: (i32, i32), works: &[Rect]) -> bool {
    let (cx, cy) = (pos.0 + size.0 / 2, pos.1 + size.1 / 2);
    works
        .iter()
        .any(|r| cx >= r.x && cx < r.right() && cy >= r.y && cy < r.bottom())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn bottom_taskbar_right_corner() {
        let work = r(0, 0, 1920, 1032);
        let tray = r(1850, 1040, 24, 40);
        assert_eq!(taskbar_edge(MON, work, tray), Edge::Bottom);
        // Centred on the icon would overflow the right edge: clamped to 1920 - 12 - 360.
        assert_eq!(
            popup_position(MON, work, tray, (360, 500), 12),
            (1548, 1032 - 500 - 12)
        );
        // An icon further left: centred on it.
        let tray = r(1000, 1040, 24, 40);
        assert_eq!(
            popup_position(MON, work, tray, (360, 500), 12).0,
            1012 - 180
        );
    }

    #[test]
    fn top_left_right_taskbars() {
        let top = r(0, 48, 1920, 1032);
        assert_eq!(
            popup_position(MON, top, r(1800, 4, 24, 40), (360, 500), 12),
            (1548, 60)
        );
        let left = r(62, 0, 1858, 1080);
        assert_eq!(
            popup_position(MON, left, r(10, 900, 40, 24), (360, 500), 12),
            (74, 1080 - 12 - 500)
        );
        let right = r(0, 0, 1858, 1080);
        assert_eq!(
            popup_position(MON, right, r(1870, 100, 40, 24), (360, 500), 12),
            (1858 - 360 - 12, 12)
        );
    }

    #[test]
    fn second_monitor_with_offset_origin() {
        // A monitor to the left of the primary, at negative coordinates, taskbar at the bottom.
        let mon = r(-2560, -200, 2560, 1440);
        let work = r(-2560, -200, 2560, 1392);
        let tray = r(-100, 1200, 24, 40);
        let (x, y) = popup_position(mon, work, tray, (360, 500), 12);
        assert_eq!((x, y), (-2560 + 2560 - 12 - 360, -200 + 1392 - 500 - 12));
    }

    #[test]
    fn auto_hide_taskbar_uses_nearest_edge() {
        assert_eq!(taskbar_edge(MON, MON, r(1800, 1070, 24, 10)), Edge::Bottom);
        assert_eq!(taskbar_edge(MON, MON, r(1800, 0, 24, 10)), Edge::Top);
    }

    #[test]
    fn saved_position_visibility() {
        let works = [r(0, 0, 1920, 1032)];
        assert!(visible_on((1600, 900), (232, 72), &works));
        assert!(!visible_on((3000, 900), (232, 72), &works));
    }
}
