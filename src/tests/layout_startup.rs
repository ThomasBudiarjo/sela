//! sunday-core fix: window layout startup (SC-LAYOUT-01, SC-LAYOUT-03).
use super::*;
use gpui::{DisplayId, Pixels};
use sela::pane_layout::Placement;

fn area(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn primary() -> (DisplayId, Bounds<Pixels>) {
    (DisplayId::new(1), area(0., 0., 2560., 1400.))
}

fn spare() -> (DisplayId, Bounds<Pixels>) {
    (DisplayId::new(2), area(2560., 0., 1920., 1040.))
}

fn opened(
    saved: Placement,
    work_areas: &[(DisplayId, Bounds<Pixels>)],
) -> (Option<WindowBounds>, Option<DisplayId>) {
    let options = operator_window_options(
        Some(saved),
        work_areas,
        Some(primary().0),
        area(640., 300., 1280., 800.),
    );
    // GPUI opens on the primary monitor when no display is given.
    let display = options.display_id.or(Some(primary().0));
    (options.window_bounds, display)
}

#[test]
fn a_window_saved_on_the_spare_monitor_opens_there_at_its_saved_size() {
    for maximized in [false, true] {
        let saved = Placement {
            x: 2700,
            y: 50,
            width: 1000,
            height: 700,
            maximized,
        };
        let bounds = area(2700., 50., 1000., 700.);
        let bounds = if maximized {
            WindowBounds::Maximized(bounds)
        } else {
            WindowBounds::Windowed(bounds)
        };
        let expected = (Some(bounds), Some(spare().0));
        assert_eq!(opened(saved, &[primary(), spare()]), expected);
        assert_eq!(
            opened(saved, &[spare(), primary()]),
            expected,
            "the spare monitor listed first"
        );
    }
}

#[test]
fn a_window_saved_on_a_disconnected_monitor_opens_centred_on_the_primary() {
    let saved = Placement {
        x: 5000,
        y: 50,
        width: 1000,
        height: 700,
        maximized: false,
    };
    assert_eq!(
        opened(saved, &[spare(), primary()]),
        (
            Some(WindowBounds::Windowed(area(780., 350., 1000., 700.))),
            Some(primary().0)
        )
    );
}
