use super::*;

impl Operator {
    pub(crate) fn dimensions(&self, width: f32, height: f32) -> [f32; 4] {
        let available = (width - 12.).max(0.);
        let minimum = 150_f32.min(available / 3.);
        let left =
            (available * self.ratios[0]).clamp(minimum, (available - 2. * minimum).max(minimum));
        let boundary = (available * self.ratios[1])
            .clamp(left + minimum, (available - minimum).max(left + minimum));
        let body = (height - 76.).max(0.);
        let upper = if self.collapsed {
            (height - 70.).max(0.)
        } else {
            (body * self.ratios[2]).clamp(
                140_f32.min(body / 2.),
                (body - 140_f32.min(body / 2.)).max(0.),
            )
        };
        [left, boundary - left, available - boundary, upper]
    }

    pub(crate) fn splitter(&self, axis: usize, cx: &mut Context<Self>) -> impl IntoElement {
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
            .bg(rgb(CHROME))
            .flex()
            .items_center()
            .justify_center()
            .hover(|d| d.bg(rgb(SPLITTER_HOVER)))
            .child(
                div()
                    .bg(rgb(BORDER))
                    .when(horizontal, |d| d.h(px(1.)).w_full())
                    .when(!horizontal, |d| d.w(px(1.)).h_full()),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, _| {
                    window.prevent_default();
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

impl Render for Operator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |this, window, _| {
                if window.is_window_active() {
                    // Songs saved in the editor window appear on return.
                    this.catalog_stale = true;
                } else {
                    this.drag = None;
                }
            }));
        }
        let viewport = window.viewport_size();
        let [left, middle, right, upper] =
            self.dimensions(viewport.width.into(), viewport.height.into());
        let live_label = if self.output.is_some() {
            "Live ● On"
        } else {
            "Live ○ Off"
        };
        div()
            .key_context("Sela SelaShow")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                if this.may_close(window, cx) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|this, _: &SaveSchedule, window, cx| {
                this.save_schedule(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSchedule, window, cx| {
                this.open_schedule(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextScheduleItem, _, cx| {
                this.step_schedule(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviousScheduleItem, _, cx| {
                this.step_schedule(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &RemoveScheduleItem, _, cx| {
                this.remove_selected();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleBlack, _, cx| {
                this.toggle_mask(Mask::Black);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleLogo, _, cx| {
                this.toggle_mask(Mask::Logo);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleClear, _, cx| {
                this.toggle_mask(Mask::Clear);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &GoLive, _, cx| {
                this.go_live();
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, cx| window.focus_prev(cx)))
            .capture_any_mouse_up(cx.listener(|this, _, _, _| this.drag = None))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(CHROME))
            .text_color(rgb(TEXT))
            .font_family("DejaVu Sans")
            .text_size(px(TEXT_BASE))
            .child(
                div()
                    .h(px(TOP_BAR_H))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(self.button(Control::NewMenu, "new-menu", "New ▾", cx))
                    .child(self.button(Control::OpenSchedule, "open-schedule", "Open", cx))
                    .child(self.button(Control::SaveSchedule, "save-schedule", "Save", cx))
                    .children(
                        ["Web", "Remote"]
                            .map(|label| div().px_1().text_color(rgb(INACTIVE)).child(label)),
                    )
                    .child(div().flex_1())
                    .child(
                        self.button(Control::GoLive, "go-live", "Go Live", cx)
                            .text_color(rgb(TEXT)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "Alerts".into())
                            .px_1()
                            .text_color(rgb(INACTIVE))
                            .child("Alerts"),
                    )
                    .child(self.mask_button(Control::Logo, "mask-logo", "Logo", Mask::Logo, cx))
                    .child(self.mask_button(Control::Black, "mask-black", "Black", Mask::Black, cx))
                    .child(self.mask_button(Control::Clear, "mask-clear", "Clear", Mask::Clear, cx))
                    .child(
                        self.button(Control::LiveOutput, "live-output", live_label, cx)
                            .when(self.output.is_some(), |d| d.text_color(rgb(ON_SCREEN))),
                    ),
            )
            .child(
                div()
                    .h(px(upper))
                    .flex_shrink_0()
                    .flex()
                    .child(self.schedule_pane(left, cx))
                    .child(self.splitter(0, cx))
                    .child(self.preview_pane(middle, cx))
                    .child(self.splitter(1, cx))
                    .child(
                        div()
                            .w(px(right))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(div().flex_1().min_h_0().child(self.live_pane(right, cx)))
                            .child(
                                div()
                                    .h(px(PANE_HEADER_H))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .border_t_1()
                                    .border_color(rgb(BORDER))
                                    .child(self.button(
                                        Control::Previous,
                                        "live-previous",
                                        "‹ Previous",
                                        cx,
                                    ))
                                    .child(self.button(Control::Next, "live-next", "Next ›", cx)),
                            ),
                    ),
            )
            .when(!self.collapsed, |d| d.child(self.splitter(2, cx)))
            .child(
                div()
                    .h(px(STATUS_BAR_H))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_3()
                    .child(div().font_weight(FontWeight::MEDIUM).child("Resources"))
                    .child(self.button(
                        Control::CollapseResources,
                        "collapse-resources",
                        if self.collapsed {
                            "Restore resources"
                        } else {
                            "Collapse resources"
                        },
                        cx,
                    ))
                    .child(self.button(Control::Quit, "quit", "Quit · Ctrl+Q", cx))
                    .child(self.button(Control::ResetLayout, "reset-layout", "Reset layout", cx))
                    .child(
                        div()
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(MUTED))
                            .child(self.output_line()),
                    ),
            )
            .when(!self.collapsed, |d| {
                let detail = self.resource_detail(cx);
                d.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(TAB_BAR_H))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .px_3()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .children(TABS.iter().enumerate().map(|(index, label)| {
                                    self.button(Control::TABS[index], label, *label, cx)
                                        .h_full()
                                        .rounded_none()
                                        .border_b_2()
                                        .border_color(rgb(if self.tab == index {
                                            ACCENT
                                        } else {
                                            CHROME
                                        }))
                                        .bg(rgb(if self.tab == index { SURFACE } else { CHROME }))
                                        .when(self.tab == index, |d| {
                                            d.text_color(rgb(TEXT)).font_weight(FontWeight::MEDIUM)
                                        })
                                })),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .bg(rgb(SURFACE))
                                .child(
                                    div()
                                        .w(px(left))
                                        .flex_shrink_0()
                                        .p_3()
                                        .border_r_1()
                                        .border_color(rgb(BORDER))
                                        .bg(rgb(CHROME))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Collections"),
                                        )
                                        .child(
                                            div()
                                                .mt_2()
                                                .text_color(rgb(MUTED))
                                                .child("No collections"),
                                        ),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "library-detail".into())
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(div().flex_1().min_h_0().child(detail))
                                        .children(self.library_error.clone().map(|error| {
                                            div().px_3().py_1().text_color(rgb(ERROR)).child(error)
                                        })),
                                ),
                        ),
                )
                .when(self.tab == 0, |d| {
                    let normalized = self.sizing == Sizing::Normalized;
                    d.child(
                        div()
                            .h(px(STATUS_BAR_H))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(self.button(
                                Control::NewSongButton,
                                "open-library",
                                "+ New Song",
                                cx,
                            ))
                            .child(
                                self.button(
                                    Control::AddToSchedule,
                                    "add-to-schedule",
                                    "Add to Schedule",
                                    cx,
                                )
                                .when(
                                    self.selected_song.is_none() || self.selected_entry.is_some(),
                                    |d| d.text_color(rgb(DISABLED)),
                                ),
                            )
                            .children(self.song_message.clone().map(|(message, failed)| {
                                div()
                                    .debug_selector(|| "song-message".into())
                                    .px_2()
                                    .min_w_0()
                                    .text_size(px(TEXT_CAPTION))
                                    .text_color(rgb(if failed { ERROR } else { MUTED }))
                                    .truncate()
                                    .child(message)
                            }))
                            .child(div().flex_1())
                            .child(
                                self.button(
                                    Control::Normalize,
                                    "normalize-text",
                                    if normalized {
                                        "Normalize text size across slides: On"
                                    } else {
                                        "Normalize text size across slides: Off"
                                    },
                                    cx,
                                )
                                .when(normalized, |d| d.text_color(rgb(TEXT))),
                            ),
                    )
                })
                .when(self.tab == 2, |d| {
                    d.child(
                        div()
                            .h(px(STATUS_BAR_H))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(self.button(
                                Control::ImportImage,
                                "import-image",
                                "Import image…",
                                cx,
                            ))
                            .child(
                                self.button(
                                    Control::UseAsLogo,
                                    "use-as-logo",
                                    "Use As Logo Background",
                                    cx,
                                )
                                .when(self.selected_image.is_none(), |d| {
                                    d.text_color(rgb(DISABLED))
                                }),
                            )
                            .children(self.media_message.clone().map(|message| {
                                div()
                                    .debug_selector(|| "media-message".into())
                                    .px_2()
                                    .text_size(px(TEXT_CAPTION))
                                    .text_color(rgb(MUTED))
                                    .truncate()
                                    .child(message)
                            }))
                            .children(self.logo_error.clone().map(|error| {
                                div()
                                    .px_2()
                                    .text_size(px(TEXT_CAPTION))
                                    .text_color(rgb(ERROR))
                                    .truncate()
                                    .child(error)
                            })),
                    )
                })
            })
            .when_some(
                self.image_menu.filter(|_| self.tab == 2 && !self.collapsed),
                |d, position| {
                    d.child(
                        div()
                            .occlude()
                            .absolute()
                            .left(position.x)
                            .top(position.y)
                            .bg(rgb(SURFACE))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .shadow_md()
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.image_menu = None;
                                cx.notify();
                            }))
                            .child(self.button(
                                Control::ImageMenuLogo,
                                "image-menu-logo",
                                "Use As Logo Background",
                                cx,
                            )),
                    )
                },
            )
            .when(self.new_menu, |d| {
                d.child(
                    div()
                        .occlude()
                        .absolute()
                        .top(px(TOP_BAR_H))
                        .left(px(8.))
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .child(self.button(Control::NewSongItem, "new-song-menu", "New Song", cx))
                        .child(self.button(
                            Control::NewSchedule,
                            "new-schedule-menu",
                            "New Schedule",
                            cx,
                        )),
                )
            })
            .when_some(self.item_menu, |d, (_, position)| {
                d.child(
                    div()
                        .occlude()
                        .absolute()
                        .left(position.x)
                        .top(position.y)
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.item_menu = None;
                            cx.notify();
                        }))
                        .child(self.button(
                            Control::ItemMenuRemove,
                            "item-menu-remove",
                            "Remove From Schedule",
                            cx,
                        )),
                )
            })
            .children(self.song_menu_overlay(cx))
            .children(self.dialog_overlay(cx))
    }
}
