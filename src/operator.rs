use super::*;
use gpui::{CursorStyle, Empty, MouseButton, Point, Subscription};

const TABS: [&str; 5] = ["Songs", "Scriptures", "Media", "Presentations", "Themes"];

#[derive(Clone, Copy)]
struct Split(usize);

pub(super) struct Operator {
    pub(super) focus: FocusHandle,
    pub(super) ratios: [f32; 3],
    pub(super) collapsed: bool,
    pub(super) tab: usize,
    drag: Option<(usize, Point<gpui::Pixels>, f32)>,
    activation: Option<Subscription>,
}

impl Operator {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            ratios: [0.24, 0.62, 0.62],
            collapsed: false,
            tab: 0,
            drag: None,
            activation: None,
        }
    }

    pub(super) fn dimensions(&self, width: f32, height: f32) -> [f32; 4] {
        let available = (width - 12.).max(0.);
        let minimum = 150_f32.min(available / 3.);
        let left =
            (available * self.ratios[0]).clamp(minimum, (available - 2. * minimum).max(minimum));
        let boundary = (available * self.ratios[1])
            .clamp(left + minimum, (available - minimum).max(left + minimum));
        let body = (height - 76.).max(0.);
        let upper = if self.collapsed {
            body
        } else {
            (body * self.ratios[2]).clamp(
                140_f32.min(body / 2.),
                (body - 140_f32.min(body / 2.)).max(0.),
            )
        };
        [left, boundary - left, available - boundary, upper]
    }

    fn splitter(&self, axis: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let horizontal = axis == 2;
        div()
            .id(match axis {
                0 => "split-schedule",
                1 => "split-live",
                _ => "split-resources",
            })
            .debug_selector(|| {
                match axis {
                    0 => "split-schedule",
                    1 => "split-live",
                    _ => "split-resources",
                }
                .into()
            })
            .flex_shrink_0()
            .when(horizontal, |d| {
                d.h(px(6.)).w_full().cursor(CursorStyle::ResizeUpDown)
            })
            .when(!horizontal, |d| {
                d.w(px(6.)).h_full().cursor(CursorStyle::ResizeLeftRight)
            })
            .bg(rgb(0xc8d0da))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, _| {
                    this.drag = Some((axis, event.position, this.ratios[axis]));
                }),
            )
            .on_drag(Split(axis), |_, _, _, cx| cx.new(|_| Empty))
            .on_drag_move::<Split>(cx.listener(
                move |this, event: &gpui::DragMoveEvent<Split>, window, cx| {
                    if let Some((active, start, ratio)) = this.drag {
                        if active != axis || active != event.drag(cx).0 {
                            return;
                        }
                        let viewport = window.viewport_size();
                        let extent = if horizontal {
                            f32::from(viewport.height) - 76.
                        } else {
                            f32::from(viewport.width) - 12.
                        };
                        let delta = if horizontal {
                            event.event.position.y - start.y
                        } else {
                            event.event.position.x - start.x
                        };
                        if extent > 0. {
                            this.ratios[axis] = (ratio + f32::from(delta) / extent).clamp(0., 1.);
                            // Normalize stored proportions to the actual bounded geometry.
                            let d = this
                                .dimensions(f32::from(viewport.width), f32::from(viewport.height));
                            if horizontal {
                                this.ratios[2] = d[3] / extent;
                            } else {
                                this.ratios[0] = d[0] / extent;
                                this.ratios[1] = (d[0] + d[1]) / extent;
                            }
                            cx.notify();
                        }
                    }
                },
            ))
    }
}

fn control(id: &'static str, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(|| id.into())
        .px_3()
        .py_1()
        .cursor_pointer()
        .bg(rgb(0xe2e7ee))
        .child(label)
}

fn pane(title: &'static str, text: &'static str, width: f32, dark: bool) -> impl IntoElement {
    div()
        .debug_selector(|| title.into())
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(32.))
                .flex_shrink_0()
                .px_3()
                .py_1()
                .bg(rgb(0xe8ecf1))
                .child(title),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .p_3()
                .bg(rgb(if dark { 0x17202e } else { 0xffffff }))
                .text_color(rgb(if dark { 0xc5cfdd } else { 0x596576 }))
                .child(text),
        )
}

impl Render for Operator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |this, window, _| {
                if !window.is_window_active() {
                    this.drag = None;
                }
            }));
        }
        let viewport = window.viewport_size();
        let [left, middle, right, upper] =
            self.dimensions(viewport.width.into(), viewport.height.into());
        div()
            .key_context("Sela")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &Quit, _, cx| cx.quit()))
            .capture_any_mouse_up(cx.listener(|this, _, _, _| this.drag = None))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(0xf4f6f9))
            .text_color(rgb(0x182536))
            .font_family("DejaVu Sans")
            .text_sm()
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_3()
                    .child(div().text_color(rgb(0x233f65)).child("Sela"))
                    .child("Development preview")
                    .child(
                        control("reset-layout", "Reset layout").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.ratios = [0.24, 0.62, 0.62];
                                this.collapsed = false;
                                this.drag = None;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        control("quit", "Quit · Ctrl+Q")
                            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
                    ),
            )
            .child(
                div()
                    .h(px(upper))
                    .flex_shrink_0()
                    .flex()
                    .child(pane("Schedule", "Schedule is empty", left, false))
                    .child(self.splitter(0, cx))
                    .child(pane("Preview", "Nothing selected", middle, true))
                    .child(self.splitter(1, cx))
                    .child(
                        div()
                            .w(px(right))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(div().flex_1().min_h_0().child(pane(
                                "Live",
                                "Output not connected",
                                right,
                                true,
                            )))
                            .child(
                                div()
                                    .h(px(32.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .px_2()
                                    .children(
                                        ["Go Live", "Black", "Clear", "Logo"].map(|label| {
                                            div().text_color(rgb(0x78818d)).child(label)
                                        }),
                                    ),
                            ),
                    ),
            )
            .when(!self.collapsed, |d| d.child(self.splitter(2, cx)))
            .child(
                div()
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_3()
                    .child("Resources")
                    .child(
                        control(
                            "collapse-resources",
                            if self.collapsed {
                                "Restore resources"
                            } else {
                                "Collapse resources"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.collapsed = !this.collapsed;
                            this.drag = None;
                            cx.notify();
                        })),
                    ),
            )
            .when(!self.collapsed, |d| {
                d.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(32.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_3()
                                .children(TABS.iter().enumerate().map(|(index, label)| {
                                    control(label, label)
                                        .bg(rgb(if self.tab == index {
                                            0xcbd9eb
                                        } else {
                                            0xe2e7ee
                                        }))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.tab = index;
                                            cx.notify();
                                        }))
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .bg(rgb(0xffffff))
                                .child(
                                    div()
                                        .w(px(left))
                                        .flex_shrink_0()
                                        .p_3()
                                        .bg(rgb(0xeef1f5))
                                        .child("Collections")
                                        .child(div().mt_2().child("No collections")),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "library-detail".into())
                                        .flex_1()
                                        .p_3()
                                        .child(format!("{} library is empty", TABS[self.tab]))
                                        .child(
                                            div()
                                                .mt_2()
                                                .text_color(rgb(0x596576))
                                                .child("Library storage is not implemented."),
                                        ),
                                ),
                        ),
                )
            })
    }
}
