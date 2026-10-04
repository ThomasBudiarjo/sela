use super::*;
use gpui::{Entity, TestAppContext, VisualTestContext};
use std::{cell::Cell, rc::Rc};

// Observe dispatch in capture phase without replacing Operator's real handler.
// TestPlatform::quit is a no-op; native process termination is checked separately.
struct DispatchProbe {
    operator: Entity<Operator>,
    seen: Rc<Cell<usize>>,
}

impl Render for DispatchProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        div()
            .size_full()
            .capture_action(move |_: &Quit, _, _| seen.set(seen.get() + 1))
            .child(self.operator.clone())
    }
}

fn fixture(cx: &mut TestAppContext) -> (VisualTestContext, Entity<Operator>, Rc<Cell<usize>>) {
    let seen = Rc::new(Cell::new(0));
    let window = cx.update(|cx| {
        bind_operator_keys(cx);
        cx.open_window(Default::default(), |window, cx| {
            let operator = cx.new(Operator::new);
            operator.read(cx).focus.clone().focus(window, cx);
            cx.new(|_| DispatchProbe {
                operator,
                seen: seen.clone(),
            })
        })
        .unwrap()
    });
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let operator = window
        .root(&mut visual)
        .unwrap()
        .read_with(&visual, |p, _| p.operator.clone());
    (visual, operator, seen)
}

#[gpui::test]
fn focused_operator_resolves_quit_input(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.update(|window, cx| {
        assert!(operator.read(cx).focus.is_focused(window));
        assert!(window.is_action_available(&Quit, cx));
    });
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "actual Sela key context resolves Quit");
}

#[gpui::test]
fn unbound_and_unfocused_input_do_not_dispatch(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.simulate_keystrokes("ctrl-j");
    assert_eq!(seen.get(), 0);
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 0, "no focused Sela context");
    cx.update(|window, cx| operator.read(cx).focus.clone().focus(window, cx));
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "restored focus restores contextual binding");
}

#[gpui::test]
fn wrong_context_rejects_key_but_direct_action_bypasses_binding(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.update(|_, cx| {
        cx.clear_key_bindings();
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, Some("NotSela"))]);
    });
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 0);
    cx.update(|window, cx| {
        operator
            .read(cx)
            .focus
            .clone()
            .dispatch_action(&Quit, window, cx);
    });
    assert_eq!(seen.get(), 1, "direct dispatch is not key-context E2E");
}

#[gpui::test]
fn shell_controls_and_resize(cx: &mut TestAppContext) {
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    for (index, tab) in ["Songs", "Scriptures", "Media", "Presentations", "Themes"]
        .into_iter()
        .enumerate()
    {
        let bounds = cx.debug_bounds(tab).unwrap();
        cx.simulate_click(bounds.center(), Default::default());
        assert_eq!(operator.read_with(&cx, |o, _| o.tab), index);
    }
    let bounds = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    assert!(operator.read_with(&cx, |o, _| o.collapsed));
    assert!(cx.debug_bounds("library-detail").is_none());
    assert_eq!(
        operator.read_with(&cx, |o, _| o.dimensions(1280., 800.))[3],
        730.
    );
    let bounds = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(bounds.center(), Default::default());
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.simulate_resize(size(px(720.), px(440.)));
    for selector in ["Schedule", "Preview", "Live", "library-detail"] {
        let b = cx.debug_bounds(selector).unwrap();
        assert!(b.size.width > px(0.) && b.size.height > px(0.));
        assert!(b.right() <= px(720.) && b.bottom() <= px(440.));
    }
    cx.update(|window, cx| {
        assert!(operator.read(cx).controls[2].is_focused(window));
        assert!(operator.read(cx).focus.contains_focused(window, cx));
    });
}

#[gpui::test]
fn splitters_bound_release_and_reset(cx: &mut TestAppContext) {
    use gpui::{MouseButton, point};
    let (mut cx, operator, _) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    for selector in ["split-schedule", "split-live", "split-resources"] {
        let old = operator.read_with(&cx, |o, _| o.ratios);
        let start = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start + point(px(15.), px(15.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(
            point(px(3000.), px(3000.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(
            point(px(3000.), px(3000.)),
            MouseButton::Left,
            Default::default(),
        );
        let before = operator.read_with(&cx, |o, _| o.ratios);
        if selector == "split-resources" {
            assert_eq!(&old[..2], &before[..2]);
        } else {
            assert_eq!(old[2], before[2], "vertical drag must not resize resources");
        }
        cx.simulate_mouse_move(point(px(0.), px(0.)), None, Default::default());
        assert_eq!(before, operator.read_with(&cx, |o, _| o.ratios));
        let d = operator.read_with(&cx, |o, _| o.dimensions(1280., 800.));
        assert!(d[..3].iter().all(|v| *v >= 149.99));
        assert!(d[3] >= 140. && d[3] <= 584.);
        cx.update(|window, cx| assert!(operator.read(cx).focus.is_focused(window)));
    }
    assert_ne!(operator.read_with(&cx, |o, _| o.ratios), [0.24, 0.62, 0.62]);
    let b = cx.debug_bounds("reset-layout").unwrap();
    cx.simulate_click(b.center(), Default::default());
    cx.update(|window, cx| assert!(operator.read(cx).controls[0].is_focused(window)));
    assert_eq!(operator.read_with(&cx, |o, _| o.ratios), [0.24, 0.62, 0.62]);
    for (selector, dimension, minimum) in [
        ("split-schedule", 0, 150.),
        ("split-live", 1, 150.),
        ("split-resources", 3, 140.),
    ] {
        let start = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
        cx.simulate_mouse_move(
            start - point(px(15.), px(15.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_move(
            point(px(-3000.), px(-3000.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(
            point(px(-3000.), px(-3000.)),
            MouseButton::Left,
            Default::default(),
        );
        let d = operator.read_with(&cx, |o, _| o.dimensions(1280., 800.));
        assert!((d[dimension] - minimum).abs() < 0.01);
        cx.update(|window, cx| {
            assert!(operator.read(cx).controls[0].is_focused(window));
            assert!(operator.read(cx).focus.contains_focused(window, cx));
        });
    }
    for (w, h) in [(0., 0.), (10., 10.), (720., 440.), (1280., 800.)] {
        let d = operator.read_with(&cx, |o, _| o.dimensions(w, h));
        assert!(d.iter().all(|v| v.is_finite() && *v >= 0.));
        assert!((d[0] + d[1] + d[2] - (w - 12.).max(0.)).abs() < 0.01);
    }
}

fn assert_control(cx: &mut VisualTestContext, operator: &Entity<Operator>, index: usize) {
    cx.update(|window, cx| {
        assert!(
            operator.read(cx).controls[index].is_focused(window),
            "control {index}"
        );
        assert!(operator.read(cx).focus.contains_focused(window, cx));
    });
}

#[gpui::test]
fn keyboard_traversal_and_activation(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    for index in 0..8 {
        cx.simulate_keystrokes("tab");
        assert_control(&mut cx, &operator, index);
        assert_eq!(operator.read_with(&cx, |o, _| o.tab), 0);
    }
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 0);
    cx.simulate_keystrokes("shift-tab");
    assert_control(&mut cx, &operator, 7);
    for index in (0..7).rev() {
        cx.simulate_keystrokes("shift-tab");
        assert_control(&mut cx, &operator, index);
    }
    cx.simulate_keystrokes("tab tab tab tab");
    assert_control(&mut cx, &operator, 4);
    cx.simulate_keystrokes("enter");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 1);
    cx.simulate_keystrokes("tab space");
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    let schedule = cx.debug_bounds("Schedule").unwrap();
    let ratios = operator.read_with(&cx, |o, _| o.ratios);
    cx.simulate_keystrokes("ctrl-j down left shift-enter shift-space");
    assert_control(&mut cx, &operator, 5);
    assert_eq!(operator.read_with(&cx, |o, _| o.tab), 2);
    assert_eq!(operator.read_with(&cx, |o, _| o.ratios), ratios);
    assert_eq!(cx.debug_bounds("Schedule").unwrap(), schedule);
    cx.simulate_keystrokes("ctrl-q");
    assert_eq!(seen.get(), 1, "root Quit routes from a child context");
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab shift-tab enter space");
    assert_control(&mut cx, &operator, 1);
    assert_eq!(
        seen.get(),
        3,
        "each focused Quit activation dispatches once"
    );
    cx.update(|_, cx| cx.clear_key_bindings());
    cx.simulate_keystrokes("enter space tab ctrl-q");
    assert_control(&mut cx, &operator, 1);
    assert_eq!(
        seen.get(),
        3,
        "unbound keys must not bypass semantic actions"
    );
}

#[gpui::test]
fn collapsed_traversal_reset_and_rejection(cx: &mut TestAppContext) {
    let (mut cx, operator, seen) = fixture(cx);
    cx.simulate_resize(size(px(1280.), px(800.)));
    let original = operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed));
    cx.simulate_keystrokes("enter space ctrl-j down left");
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    cx.simulate_keystrokes("tab tab tab enter");
    assert!(operator.read_with(&cx, |o, _| o.collapsed));
    assert_control(&mut cx, &operator, 2);
    assert!(cx.debug_bounds("Songs").is_none());
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 0);
    cx.simulate_keystrokes("shift-tab space");
    assert_control(&mut cx, &operator, 2);
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.simulate_keystrokes("tab");
    assert_control(&mut cx, &operator, 3);
    let b = cx.debug_bounds("collapse-resources").unwrap();
    cx.simulate_click(b.center(), Default::default());
    assert_control(&mut cx, &operator, 2);
    cx.simulate_keystrokes("tab enter");
    assert_control(&mut cx, &operator, 0);
    assert!(!operator.read_with(&cx, |o, _| o.collapsed));
    cx.update(|window, cx| window.blur(cx));
    cx.simulate_keystrokes("tab enter space ctrl-q");
    cx.update(|window, cx| assert!(!operator.read(cx).focus.contains_focused(window, cx)));
    assert_eq!(seen.get(), 0);
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    // Unavailable Live labels have neither focus handles nor activation routes.
    for label in ["Go Live", "Black", "Clear", "Logo"] {
        let bounds = cx.debug_bounds(label).unwrap();
        cx.simulate_click(bounds.center(), Default::default());
        cx.update(|window, cx| {
            assert!(operator.read(cx).focus.is_focused(window));
            assert!(!window.is_action_available(&ActivateControl, cx));
        });
        cx.simulate_keystrokes("enter space");
    }
    assert_eq!(
        original,
        operator.read_with(&cx, |o, _| (o.tab, o.ratios, o.collapsed))
    );
    assert_eq!(seen.get(), 0);
}
