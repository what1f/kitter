use super::*;

impl KitterApp {
    pub(super) fn selected_skill(&self) -> Option<&SkillSummary> {
        let storage_name = self.skills_view.selection.primary()?;
        self.model
            .skills
            .iter()
            .find(|skill| skill_storage_name(skill) == storage_name)
    }

    pub(super) fn selected_skill_keys(&self) -> Vec<String> {
        let order = self
            .model
            .skills
            .iter()
            .map(|skill| skill_storage_name(skill).to_string())
            .collect::<Vec<_>>();
        self.skills_view.selection.selected_in(&order)
    }

    pub(super) fn selected_library_targets(&self) -> Vec<(String, PathBuf)> {
        let selected = self
            .selected_skill_keys()
            .into_iter()
            .collect::<HashSet<_>>();
        self.model
            .skills
            .iter()
            .filter(|skill| selected.contains(skill_storage_name(skill)))
            .map(|skill| (skill_storage_name(skill).to_string(), skill.path.clone()))
            .collect()
    }

    pub(super) fn set_detail_selection(&mut self, storage_name: Option<String>) {
        self.skills_view.selection.set_primary(storage_name);
        self.skills_view.tab = DetailTab::Installs;
        self.skills_view.selected_file = PathBuf::from("SKILL.md");
        self.skills_view.collapsed_content_directories.clear();
        *self.skills_view.content_snapshot.borrow_mut() = None;
        self.skills_view
            .content_scroll
            .set_offset(point(px(0.), px(0.)));
    }

    pub(super) fn finish_skill_selection_mode(&mut self, cx: &mut Context<Self>) {
        let order = self
            .model
            .skills
            .iter()
            .map(|skill| skill_storage_name(skill).to_string())
            .collect::<Vec<_>>();
        let primary = self.skills_view.selection.finish(&order);
        self.set_detail_selection(primary);
        cx.notify();
    }

    pub(super) fn clear_skill_selection(&mut self, cx: &mut Context<Self>) {
        self.skills_view.selection.clear();
        self.set_detail_selection(None);
        cx.notify();
    }

    pub(super) fn select_all_visible_skills(&mut self, visible: &[String], cx: &mut Context<Self>) {
        let detail = self.skills_view.selection.select_all(visible);
        self.set_detail_selection(detail);
        cx.notify();
    }

    pub(super) fn toggle_skill_selection(&mut self, storage_name: String, cx: &mut Context<Self>) {
        let order = self
            .model
            .skills
            .iter()
            .map(|skill| skill_storage_name(skill).to_string())
            .collect::<Vec<_>>();
        let primary = self.skills_view.selection.toggle(storage_name, &order);
        self.set_detail_selection(primary);
        cx.notify();
    }

    pub(super) fn select_skill_from_click(
        &mut self,
        storage_name: String,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if modifiers.secondary() {
            self.toggle_skill_selection(storage_name, cx);
        } else {
            let primary = self.skills_view.selection.select_one(storage_name);
            self.set_detail_selection(primary);
        }
        cx.notify();
    }

    pub(super) fn prepare_skill_context_selection(
        &mut self,
        storage_name: String,
        cx: &mut Context<Self>,
    ) {
        let primary = self.skills_view.selection.select_for_context(storage_name);
        self.set_detail_selection(primary);
        cx.notify();
    }
}
