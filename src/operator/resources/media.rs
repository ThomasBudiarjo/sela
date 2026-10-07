use super::*;

impl Operator {
    pub(crate) fn submit_image_job(&mut self, job: images::Job) {
        let Some(worker) = &self.image_worker else {
            self.media_message = Some("No profile folder; images are unavailable".into());
            return;
        };
        if let Err(error) = worker.submit(job) {
            self.media_message = Some(error.to_string());
        }
    }

    pub(crate) fn prompt_import(&mut self, cx: &mut Context<Self>) {
        if self.image_worker.is_none() {
            self.media_message = Some("No profile folder; images cannot be imported".into());
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.submit_image_job(images::Job::Import(path));
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn use_as_logo(&mut self) {
        match self.selected_image.clone() {
            Some(name) => {
                self.media_message = None;
                self.submit_image_job(images::Job::UseAsLogo(name));
            }
            None => self.media_message = Some("Select an image first".into()),
        }
    }

    pub(crate) fn image_list(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        while self.image_rows.len() < self.images.len() {
            let index = self.image_rows.len() as isize;
            self.image_rows.push(
                cx.focus_handle()
                    .tab_index(IMAGE_TAB_INDEX + index)
                    .tab_stop(true),
            );
        }
        if self.profile.is_none() {
            return empty_detail(
                "Images · no profile",
                "Launch Sela without arguments to use the default profile.",
            );
        }
        if self.images.is_empty() {
            return empty_detail(
                "Images",
                "No images yet. Use Import image… to add a PNG or JPEG.",
            );
        }
        let logo = self.logo.as_ref().map(|logo| logo.name.as_str());
        div()
            .id("image-list")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(self.images.iter().enumerate().map(|(index, name)| {
                let selected = self.selected_image.as_ref() == Some(name);
                let label = if logo == Some(name.as_str()) {
                    format!("{name} · Logo")
                } else {
                    name.clone()
                };
                let select = move |this: &mut Self| {
                    this.selected_image = this.images.get(index).cloned();
                };
                div()
                    .id(("image", index))
                    .debug_selector(move || format!("image-{index}"))
                    .track_focus(&self.image_rows[index])
                    .key_context("SelaControl")
                    .flex_shrink_0()
                    .h(px(28.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .cursor_pointer()
                    .bg(rgb(if selected { SELECTED_ROW } else { SURFACE }))
                    .hover(|d| d.bg(rgb(HOVER)))
                    .focus(|d| d.bg(rgb(FOCUS_BG)))
                    .on_action(cx.listener(move |this, _: &ActivateControl, window, cx| {
                        window.prevent_default();
                        select(this);
                        cx.notify();
                    }))
                    .on_click(
                        cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                            if matches!(event, gpui::ClickEvent::Keyboard(_))
                                || event.is_right_click()
                            {
                                return;
                            }
                            this.image_rows[index].focus(window, cx);
                            select(this);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            this.image_rows[index].focus(window, cx);
                            select(this);
                            this.image_menu = Some(event.position);
                            cx.notify();
                        }),
                    )
                    .child(div().truncate().child(label))
            }))
            .into_any_element()
    }
}
