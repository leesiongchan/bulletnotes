//! Display-independent geometry in AppKit logical screen coordinates.
use crate::store::settings::WindowPlacement;
#[derive(Clone, Copy, Debug)]
pub struct ScreenFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl ScreenFrame {
    fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }
}
pub fn restore(saved: &WindowPlacement, screens: &[ScreenFrame]) -> Option<WindowPlacement> {
    if ![saved.x, saved.y, saved.width, saved.height]
        .iter()
        .all(|v| v.is_finite())
        || saved.width < 400.0
        || saved.height < 440.0
    {
        return None;
    }
    let intersection = |screen: &ScreenFrame| {
        ((saved.x + saved.width).min(screen.x + screen.width) - saved.x.max(screen.x)).max(0.0)
            * ((saved.y + saved.height).min(screen.y + screen.height) - saved.y.max(screen.y))
                .max(0.0)
    };
    let mut screens = screens.iter().filter(|screen| screen.valid());
    let mut screen = screens.next()?;
    for candidate in screens {
        if intersection(candidate) > intersection(screen) {
            screen = candidate;
        }
    }
    let width = saved.width.min(screen.width).max(400.0);
    let height = saved.height.min(screen.height).max(440.0);
    Some(WindowPlacement {
        x: saved
            .x
            .clamp(screen.x, (screen.x + screen.width - width).max(screen.x)),
        y: saved
            .y
            .clamp(screen.y, (screen.y + screen.height - height).max(screen.y)),
        width,
        height,
        pinned: saved.pinned,
    })
}
