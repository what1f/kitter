use super::*;

impl KitterApp {
    pub(super) fn browse_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                    this.model.library.config.library_dir = path;
                    let _ = this.model.library.save();
                    this.refresh(cx);
                });
            }
        })
        .detach();
    }
}
