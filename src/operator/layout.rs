use super::*;
use crate::song_library::{EditorPromptOpen, editor_prompt_open};

impl Operator {
    pub(crate) fn dimensions(&self, width: f32, height: f32) -> [f32; 4] {
        let available = (width - 12.).max(0.);
        let minimum = 150_f32.min(available / 3.);
        let left =
            (available * self.ratios[0]).clamp(minimum, (available - 2. * minimum).max(minimum));
        let boundary = (available * self.ratios[1])
            .clamp(left + minimum, (available - minimum).max(left + minimum));
        let body = (height - 76. - MENU_BAR_H).max(0.);
        let upper = if self.collapsed {
            (height - 70. - MENU_BAR_H).max(0.)
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
                            f32::from(viewport.height) - 76. - MENU_BAR_H
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
        if let Some(log) = &mut self.frame_log {
            log.tick();
        }
        self.focus_pending_dialog(window, cx);
        self.focus_live_if_pending(window, cx);
        self.resume_guarded(window, cx);
        self.sync_window_title(window);
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |this, window, _| {
                if window.is_window_active() {
                    // Songs saved in the editor window appear on return.
                    this.catalog_stale = true;
                } else {
                    this.drag = None;
                }
            }));
            self.watch_window(window, cx);
        }
        let viewport = window.viewport_size();
        let [left, middle, right, upper] =
            self.dimensions(viewport.width.into(), viewport.height.into());
        let live_label = if self.output.is_some() {
            t!(Msg::LiveOn)
        } else {
            t!(Msg::LiveOff)
        };
        div()
            // Live shortcuts must not reach the main window behind a modal dialog.
            .key_context(if self.dialog.is_some() {
                "Sela"
            } else {
                "Sela SelaShow"
            })
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Quit, window, cx| {
                if this.may_close(window, cx) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|this, _: &SaveSchedule, _, cx| {
                this.save_schedule(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenSchedule, window, cx| {
                this.open_schedule(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NextScheduleItem, _, cx| {
                this.step_schedule(true, false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviousScheduleItem, _, cx| {
                this.step_schedule(false, false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ExtendNextScheduleItem, _, cx| {
                this.step_schedule(true, true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ExtendPreviousScheduleItem, _, cx| {
                this.step_schedule(false, true);
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && this.cancel_entry_drag(window, cx) {
                    cx.stop_propagation();
                }
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
                // Later PageDown/PageUp, arrows and label keys then drive
                // Live, which is how a presenter remote advances.
                this.go_live_requested();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LiveNextSlide, _, cx| {
                this.step_live(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LivePreviousSlide, _, cx| {
                this.step_live(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LiveFirstSlide, _, cx| {
                this.live_edge(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LiveLastSlide, _, cx| {
                this.live_edge(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LiveNextItem, _, cx| {
                this.step_live_item(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LivePreviousItem, _, cx| {
                this.step_live_item(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, jump: &LabelJump, _, cx| {
                this.jump_label(jump.key, jump.number);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ScheduleFirstItem, _, cx| {
                this.select_schedule_edge(false);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ScheduleLastItem, _, cx| {
                this.select_schedule_edge(true);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviewPreviousSlide, window, cx| {
                if let Some(index) = this.step_preview(false) {
                    this.slide_rows[index].focus(window, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PreviewNextSlide, window, cx| {
                if let Some(index) = this.step_preview(true) {
                    this.slide_rows[index].focus(window, cx);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &OpenFileMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::File, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenEditMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::Edit, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenLiveMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::Live, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenProfilesMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::Profiles, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenViewMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::View, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenHelpMenu, window, cx| {
                this.toggle_bar_menu(TopMenu::Help, true, window, cx);
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
            .child(self.menu_bar(cx))
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
                    .child(self.button(Control::NewMenu, "new-menu", t!(Msg::ToolbarNewMenu), cx))
                    .child(self.button(
                        Control::OpenSchedule,
                        "open-schedule",
                        t!(Msg::ToolbarOpen),
                        cx,
                    ))
                    .child(self.button(
                        Control::SaveSchedule,
                        "save-schedule",
                        t!(Msg::ToolbarSave),
                        cx,
                    ))
                    .children(
                        [Msg::ToolbarWeb, Msg::ToolbarRemote]
                            .map(|msg| div().px_1().text_color(rgb(INACTIVE)).child(t!(msg))),
                    )
                    .child(div().flex_1())
                    .child(
                        self.button(Control::GoLive, "go-live", t!(Msg::ToolbarGoLive), cx)
                            .text_color(rgb(TEXT)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "Alerts".into())
                            .px_1()
                            .text_color(rgb(INACTIVE))
                            .child(t!(Msg::ToolbarAlerts)),
                    )
                    .child(self.mask_button(
                        Control::Logo,
                        "mask-logo",
                        t!(Msg::MaskLogo),
                        Mask::Logo,
                        cx,
                    ))
                    .child(self.mask_button(
                        Control::Black,
                        "mask-black",
                        t!(Msg::MaskBlack),
                        Mask::Black,
                        cx,
                    ))
                    .child(self.mask_button(
                        Control::Clear,
                        "mask-clear",
                        t!(Msg::MaskClear),
                        Mask::Clear,
                        cx,
                    ))
                    .child(
                        self.button(Control::LiveOutput, "live-output", live_label, cx)
                            .aria_toggled(if self.output.is_some() {
                                Toggled::True
                            } else {
                                Toggled::False
                            })
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
                                        t!(Msg::ButtonPrevious),
                                        cx,
                                    ))
                                    .child(self.button(
                                        Control::Next,
                                        "live-next",
                                        t!(Msg::ButtonNext),
                                        cx,
                                    )),
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
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!(Msg::ResourcesHeader)),
                    )
                    .child(self.button(
                        Control::CollapseResources,
                        "collapse-resources",
                        if self.collapsed {
                            t!(Msg::RestoreResources)
                        } else {
                            t!(Msg::CollapseResources)
                        },
                        cx,
                    ))
                    .child(self.button(Control::Quit, "quit", t!(Msg::ButtonQuit), cx))
                    .child(self.button(
                        Control::ResetLayout,
                        "reset-layout",
                        t!(Msg::ResetLayout),
                        cx,
                    ))
                    .child(
                        div()
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(MUTED))
                            .child(self.output_line()),
                    )
                    .children(self.settings_note().map(|note| {
                        div()
                            .debug_selector(|| "settings-note".into())
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(TEXT_CAPTION))
                            .text_color(rgb(SENDING))
                            .child(note)
                    })),
            )
            .when(!self.collapsed, |d| {
                let detail = self.resource_detail(cx);
                let sidebar = (self.tab == 1).then(|| self.scripture_sidebar(cx));
                let scripture_footer = (self.tab == 1).then(|| self.scripture_footer(cx));
                let collections = sidebar.is_none();
                d.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .id("resources-tabs")
                                .role(Role::TabList)
                                .aria_label(t!(Msg::ResourcesHeader))
                                .accessibility_id("resources-tabs")
                                .h(px(TAB_BAR_H))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .px_3()
                                .border_b_1()
                                .border_color(rgb(BORDER))
                                .children(TABS.iter().enumerate().map(|(index, &msg)| {
                                    self.button(Control::TABS[index], TAB_IDS[index], t!(msg), cx)
                                        .aria_selected(self.tab == index)
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
                                        .when(collections, |d| {
                                            d.child(
                                                div()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(t!(Msg::CollectionsHeader)),
                                            )
                                            .child(
                                                div()
                                                    .mt_2()
                                                    .text_color(rgb(MUTED))
                                                    .child(t!(Msg::NoCollections)),
                                            )
                                        })
                                        .children(sidebar),
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
                .children(scripture_footer)
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
                                t!(Msg::NewSongButton),
                                cx,
                            ))
                            .child(
                                self.button(
                                    Control::AddToSchedule,
                                    "add-to-schedule",
                                    t!(Msg::AddToSchedule),
                                    cx,
                                )
                                .when(
                                    self.selected_song.is_none() || self.selected_entry.is_some(),
                                    |d| mark_disabled(d).text_color(rgb(DISABLED)),
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
                                        t!(Msg::NormalizeOn)
                                    } else {
                                        t!(Msg::NormalizeOff)
                                    },
                                    cx,
                                )
                                .when(normalized, |d| d.text_color(rgb(TEXT))),
                            ),
                    )
                })
                .when(self.tab == 2, |d| {
                    d.when(!self.assets.import_notes.is_empty(), |d| {
                        d.child(
                            div()
                                .id("import-notes")
                                .debug_selector(|| "import-notes".into())
                                .role(Role::Status)
                                .aria_label(t!(Msg::ImportImage))
                                .accessibility_id("import-notes")
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .px_3()
                                .py_1()
                                .border_t_1()
                                .border_color(rgb(BORDER))
                                .text_size(px(TEXT_CAPTION))
                                .text_color(rgb(MUTED))
                                .children(self.assets.import_notes.iter().cloned()),
                        )
                    })
                    .child(
                        div()
                            .h(px(STATUS_BAR_H))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_1()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(
                                self.button(
                                    Control::ImportImage,
                                    "import-image",
                                    t!(Msg::ImportImage),
                                    cx,
                                )
                                .when(self.assets.importing.is_some(), |d| {
                                    mark_disabled(d).text_color(rgb(DISABLED))
                                }),
                            )
                            .when(self.assets.importing.is_some(), |d| {
                                d.child(self.button(
                                    Control::CancelImport,
                                    "cancel-import",
                                    t!(Msg::ImportCancel),
                                    cx,
                                ))
                            })
                            .child(
                                self.button(
                                    Control::UseAsLogo,
                                    "use-as-logo",
                                    t!(Msg::UseAsLogo),
                                    cx,
                                )
                                .when(self.selected_image.is_none(), |d| {
                                    mark_disabled(d).text_color(rgb(DISABLED))
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
                            .id("image-menu-popup")
                            .role(Role::Menu)
                            .aria_label(t!(Msg::A11yImageMenu))
                            .accessibility_id("image-menu-popup")
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
                                t!(Msg::UseAsLogo),
                                cx,
                            )),
                    )
                },
            )
            .when(self.new_menu, |d| {
                d.child(
                    div()
                        .id("new-menu-popup")
                        .role(Role::Menu)
                        .aria_label(t!(Msg::A11yNewMenu))
                        .accessibility_id("new-menu-popup")
                        .occlude()
                        .absolute()
                        .top(px(MENU_BAR_H + TOP_BAR_H))
                        .left(px(8.))
                        .bg(rgb(SURFACE))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .shadow_md()
                        .child(self.button(
                            Control::NewSongItem,
                            "new-song-menu",
                            t!(Msg::NewSongItem),
                            cx,
                        ))
                        .child(self.button(
                            Control::NewSchedule,
                            "new-schedule-menu",
                            t!(Msg::NewScheduleItem),
                            cx,
                        )),
                )
            })
            .when_some(self.item_menu, |d, (_, position)| {
                d.child(
                    div()
                        .id("item-menu-popup")
                        .role(Role::Menu)
                        .aria_label(t!(Msg::A11yScheduleItemMenu))
                        .accessibility_id("item-menu-popup")
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
                            Control::ItemMenuDuplicate,
                            "item-menu-duplicate",
                            t!(Msg::ScheduleDuplicate),
                            cx,
                        ))
                        .child(self.button(
                            Control::ItemMenuRemove,
                            "item-menu-remove",
                            t!(Msg::RemoveFromSchedule),
                            cx,
                        )),
                )
            })
            .children(self.song_menu_overlay(cx))
            .children(self.bar_menu_overlay(cx))
            .children(self.dialog_overlay(cx))
            .children(self.options_overlay(cx))
            .when(editor_prompt_open(cx), |d| {
                d.child(div().occlude().absolute().size_full())
            })
    }
}

impl Operator {
    /// First render only: window subscriptions, detached because they last
    /// as long as the operator (the keystroke interceptor checks its window).
    fn watch_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A Songs row that scrolled out of the list or was deleted leaves
        // focus on an element GPUI no longer renders. Keys then dispatch from
        // the root view, which has no key context, so every operator key is
        // dead. The list takes that focus and keeps the Songs keys.
        cx.on_focus_lost(window, |this, window, cx| {
            let list = &this.controls[Control::SongList];
            if window.focus_lost_restore_target(cx).as_ref() == Some(list) {
                list.focus(window, cx);
            }
        })
        .detach();
        // While a song editor's close prompt is open, a backdrop holds back
        // the pointer and no keystroke reaches this window's bindings, so no
        // focus or key context can change the audience (VAL-FOCUS-001).
        cx.observe_global::<EditorPromptOpen>(|_, cx| cx.notify())
            .detach();
        let operator = window.window_handle();
        cx.intercept_keystrokes(move |_, window, cx| {
            if window.window_handle() == operator && editor_prompt_open(cx) {
                cx.stop_propagation();
            }
        })
        .detach();
    }
}
