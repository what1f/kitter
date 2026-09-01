use super::*;

impl KitterApp {
    pub(super) fn browse_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.tr("选择", "Choose").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    this.projects_view.open_project = Some(path.clone());
                    if !this.install_flow.modal {
                        this.projects_view.global_project_view = false;
                        this.projects_view.selected_project_agent = None;
                        this.projects_view.project_agents_expanded = false;
                    }
                    this.model.library.config.remember_project(path);
                    let _ = this.model.library.save();
                    if this.install_flow.modal {
                        this.install_flow.global =
                            dirs::home_dir().as_ref() == this.projects_view.open_project.as_ref();
                        this.notify_dialog(cx);
                    } else {
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
}
