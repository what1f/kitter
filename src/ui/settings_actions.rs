use super::*;

impl KitterApp {
    pub(super) fn browse_data_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("common.choose").to_string().into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(target) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    match crate::data_location::inspect(&target) {
                        Ok(kind) => {
                            this.shell.data_location = Some(state::DataLocationPrompt {
                                target,
                                kind,
                                moving: false,
                            });
                            this.open_dialog(DialogKind::DataLocation, cx);
                        }
                        Err(error) => this.show_notice(messages::error_message(&error), cx),
                    }
                });
            }
        })
        .detach();
    }

    pub(super) fn confirm_data_location(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self
            .shell
            .data_location
            .as_mut()
            .filter(|prompt| !prompt.moving)
        else {
            return;
        };
        prompt.moving = true;
        let target = prompt.target.clone();
        self.notify_dialog(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::data_location::change(&target) })
                .await;
            match result {
                Ok((_, report)) => {
                    let stale = !report.stale_links.is_empty();
                    // Every view reads the new folder from scratch.
                    let _ = cx.update(|cx| {
                        reload_windows(cx, move |app, cx| {
                            app.shell.page = Page::Settings;
                            if stale {
                                app.show_notice(t!("settings.data_stale_links").to_string(), cx);
                            }
                            cx.notify();
                        })
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.shell.data_location = None;
                        this.close_dialog(cx);
                        this.show_notice(messages::error_message(&error), cx);
                    });
                }
            }
        })
        .detach();
    }
}
