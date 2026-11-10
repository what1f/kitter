use super::*;
use crate::preset::{
    AgentsMdState, FileConflictDecision, FileDecisions, FileState, Preset, PresetStatus,
    PresetStore, SyncReport,
};

const AGENTS_MD: &str = "AGENTS.md";

/// An install or sync that stopped because project files already exist.
pub(super) enum ConflictAction {
    Install {
        id: String,
        project: PathBuf,
        targets: Vec<InstallTarget>,
        plan: project::InstallPlan,
    },
    Sync {
        id: String,
        projects: Vec<PathBuf>,
        overwrite: bool,
    },
}

impl KitterApp {
    pub(super) fn reload_presets(&mut self) {
        // The CLI writes the same store, so pick up its changes on refresh.
        if let Ok(store) = PresetStore::open_in(self.model.library.data_dir()) {
            self.presets_view.store = store;
        }
        self.presets_view.statuses.borrow_mut().clear();
        // Reload AGENTS.md on the next render in case it changed elsewhere.
        *self.presets_view.agents_md_for.borrow_mut() = None;
        let presets = self.presets_view.store.presets();
        if !self
            .presets_view
            .selected
            .as_ref()
            .is_some_and(|id| presets.iter().any(|preset| &preset.id == id))
        {
            self.presets_view.selected = presets.first().map(|preset| preset.id.clone());
        }
    }

    pub(super) fn selected_preset(&self) -> Option<&Preset> {
        let id = self.presets_view.selected.as_deref()?;
        self.presets_view.store.preset(id).ok()
    }

    pub(super) fn select_preset(&mut self, id: String, cx: &mut Context<Self>) {
        if self.presets_view.selected.as_deref() != Some(id.as_str()) {
            self.presets_view.selected_file = PathBuf::from(AGENTS_MD);
            self.presets_view.collapsed_dirs.clear();
        }
        self.presets_view.selected = Some(id);
        cx.notify();
    }

    pub(super) fn select_preset_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.save_preset_agents_md(cx);
        self.presets_view.selected_file = path;
        cx.notify();
    }

    pub(super) fn preset_status(&self, id: &str, project: &Path) -> Option<PresetStatus> {
        let key = (id.to_string(), project.to_path_buf());
        if let Some(status) = self.presets_view.statuses.borrow().get(&key) {
            return status.clone();
        }
        let status = self
            .presets_view
            .store
            .status(id, project, &self.model.skills)
            .ok();
        self.presets_view
            .statuses
            .borrow_mut()
            .insert(key, status.clone());
        status
    }

    pub(super) fn preset_needs_sync(&self, id: &str, project: &Path) -> bool {
        self.preset_status(id, project)
            .is_some_and(|status| status.needs_sync())
    }

    pub(super) fn open_preset_name_dialog(
        &mut self,
        edit: PresetEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = match &edit {
            PresetEdit::Create | PresetEdit::NewFile(_) => String::new(),
            PresetEdit::Rename(id) => self
                .presets_view
                .store
                .preset(id)
                .map(|preset| preset.name.clone())
                .unwrap_or_default(),
            PresetEdit::RenameFile(_, path) => path.to_string_lossy().into_owned(),
        };
        let placeholder = match &edit {
            PresetEdit::Create | PresetEdit::Rename(_) => t!("presets.name_placeholder"),
            PresetEdit::NewFile(_) | PresetEdit::RenameFile(..) => {
                t!("presets.file_path_placeholder")
            }
        };
        self.presets_view.name_input.update(cx, |input, cx| {
            input.set_placeholder(placeholder.to_string(), window, cx);
            input.set_value(value, window, cx);
        });
        self.presets_view.edit = Some(edit);
        self.presets_view.error = None;
        self.open_dialog(DialogKind::PresetName, cx);
        window.focus(&self.presets_view.name_input.focus_handle(cx), cx);
    }

    pub(super) fn commit_preset_name(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.presets_view.edit.clone() else {
            return;
        };
        let name = self
            .presets_view
            .name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let result = match &edit {
            PresetEdit::Create => self
                .presets_view
                .store
                .create(&name)
                .map(|preset| Some(preset.id)),
            PresetEdit::Rename(id) => self.presets_view.store.rename(id, &name).map(|_| None),
            PresetEdit::NewFile(id) => self.create_preset_file(id, Path::new(&name)).map(|_| None),
            PresetEdit::RenameFile(id, from) => {
                let to = PathBuf::from(&name);
                let result = if to == *from {
                    Ok(())
                } else if self.preset_file_exists(id, &to) {
                    Err(anyhow::anyhow!(t!("presets.file_exists").to_string()))
                } else {
                    self.save_preset_agents_md(cx);
                    self.presets_view.store.rename_file(id, from, &to)
                };
                result.map(|_| {
                    if self.presets_view.selected_file == *from {
                        self.presets_view.selected_file = to;
                    }
                    *self.presets_view.agents_md_for.borrow_mut() = None;
                    None
                })
            }
        };
        match result {
            Ok(created) => {
                if let Some(id) = created {
                    self.presets_view.selected = Some(id);
                    self.presets_view.tab = PresetTab::Skills;
                    self.presets_view.selected_file = PathBuf::from(AGENTS_MD);
                }
                self.close_dialog(cx);
                self.refresh(cx);
            }
            Err(error) => {
                self.presets_view.error = Some(self.error_message(error));
                self.notify_dialog(cx);
            }
        }
    }

    pub(super) fn open_preset_skills_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_preset().is_none() {
            return;
        }
        self.presets_view.picked_skills.clear();
        self.presets_view.skill_search.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.open_dialog(DialogKind::PresetSkills, cx);
        window.focus(&self.presets_view.skill_search.focus_handle(cx), cx);
    }

    pub(super) fn add_picked_preset_skills(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.presets_view.selected.clone() else {
            return;
        };
        let picked = self
            .model
            .skills
            .iter()
            .map(skill_storage_name)
            .filter(|name| self.presets_view.picked_skills.contains(*name))
            .map(str::to_string)
            .collect::<Vec<_>>();
        let result = self.presets_view.store.add_skills(&id, &picked);
        self.close_dialog(cx);
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    /// Removes the skills from the preset when it already holds all of them,
    /// otherwise adds the missing ones.
    pub(super) fn toggle_skills_in_preset(
        &mut self,
        id: &str,
        storage_names: &[String],
        included: bool,
        cx: &mut Context<Self>,
    ) {
        let store = &mut self.presets_view.store;
        let result = if included {
            store.remove_skills(id, storage_names)
        } else {
            store.add_skills(id, storage_names)
        };
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    pub(super) fn remove_preset_skill(
        &mut self,
        id: &str,
        storage_name: &str,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .presets_view
            .store
            .remove_skills(id, &[storage_name.to_string()]);
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    /// The preset's instruction files, AGENTS.md first.
    pub(super) fn preset_files(&self, id: &str) -> Vec<PathBuf> {
        let mut files = vec![PathBuf::from(AGENTS_MD)];
        files.extend(self.presets_view.store.files(id).unwrap_or_default());
        files
    }

    fn preset_file_exists(&self, id: &str, path: &Path) -> bool {
        self.preset_files(id).iter().any(|file| file == path)
    }

    fn read_preset_file(&self, id: &str, path: &Path) -> anyhow::Result<String> {
        if path == Path::new(AGENTS_MD) {
            self.presets_view.store.agents_md(id)
        } else {
            self.presets_view.store.read_file(id, path)
        }
    }

    fn create_preset_file(&mut self, id: &str, path: &Path) -> anyhow::Result<()> {
        if self.preset_file_exists(id, path) {
            anyhow::bail!(t!("presets.file_exists").to_string());
        }
        self.presets_view.store.write_file(id, path, "")?;
        self.reload_preset_editor();
        self.presets_view.selected_file = path.to_path_buf();
        Ok(())
    }

    /// Adds dropped files, or every file inside dropped folders, keeping the
    /// folder name as the first path component.
    /// Move a file or folder of the preset into `target_dir` (empty for the root).
    pub(super) fn move_preset_entry(
        &mut self,
        id: String,
        drag: &PresetFileDrag,
        target_dir: &Path,
        cx: &mut Context<Self>,
    ) {
        let source = drag.path.as_path();
        let Some(name) = source.file_name() else {
            return;
        };
        if source.parent() == Some(target_dir) || (drag.directory && target_dir.starts_with(source))
        {
            return;
        }
        let destination = target_dir.join(name);
        let moves = self
            .preset_files(&id)
            .into_iter()
            .filter_map(|file| {
                let to = if drag.directory {
                    destination.join(file.strip_prefix(source).ok()?)
                } else if file == source {
                    destination.clone()
                } else {
                    return None;
                };
                Some((file, to))
            })
            .collect::<Vec<_>>();
        if moves.iter().any(|(_, to)| self.preset_file_exists(&id, to)) {
            self.show_notice(t!("presets.file_exists").to_string(), cx);
            return;
        }
        self.save_preset_agents_md(cx);
        let mut error = None;
        for (from, to) in &moves {
            match self.presets_view.store.rename_file(&id, from, to) {
                Ok(()) => {
                    if self.presets_view.selected_file == *from {
                        self.presets_view.selected_file = to.clone();
                    }
                }
                Err(failure) => {
                    error.get_or_insert(failure);
                }
            }
        }
        *self.presets_view.agents_md_for.borrow_mut() = None;
        self.presets_view.statuses.borrow_mut().clear();
        if let Some(error) = error {
            self.show_notice(self.error_message(error), cx);
        }
        self.refresh(cx);
    }

    /// Import dropped files or folders into `target_dir` (empty for the root).
    pub(super) fn add_dropped_preset_files(
        &mut self,
        id: String,
        paths: &[PathBuf],
        target_dir: &Path,
        cx: &mut Context<Self>,
    ) {
        let mut added = None;
        let mut error = None;
        for source in paths {
            let Some(base) = source.parent() else {
                continue;
            };
            let files = walkdir::WalkDir::new(source)
                .sort_by_file_name()
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file());
            for entry in files {
                let Ok(relative) = entry.path().strip_prefix(base) else {
                    continue;
                };
                let relative = target_dir.join(relative);
                let relative = relative.as_path();
                let result = std::fs::read(entry.path())
                    .map_err(anyhow::Error::from)
                    .and_then(|content| self.presets_view.store.write_file(&id, relative, content));
                match result {
                    Ok(()) => {
                        added.get_or_insert_with(|| relative.to_path_buf());
                    }
                    Err(failure) => {
                        error.get_or_insert(failure);
                    }
                }
            }
        }
        if let Some(path) = added {
            self.save_preset_agents_md(cx);
            self.presets_view.selected_file = path;
            *self.presets_view.agents_md_for.borrow_mut() = None;
            self.presets_view.statuses.borrow_mut().clear();
        }
        if let Some(error) = error {
            self.show_notice(self.error_message(error), cx);
        }
        cx.notify();
    }

    /// Load the selected preset file into the editor when the selection
    /// changes. Called while rendering the instructions tab.
    pub(super) fn load_preset_agents_md(&self, id: &str, window: &mut Window, cx: &mut App) {
        let key = (id.to_string(), self.presets_view.selected_file.clone());
        if self.presets_view.agents_md_for.borrow().as_ref() == Some(&key) {
            return;
        }
        let content = self.read_preset_file(id, &key.1).unwrap_or_default();
        self.presets_view.agents_md.update(cx, |editor, cx| {
            if editor.value().as_ref() != content {
                editor.set_value(content, window, cx);
            }
        });
        *self.presets_view.agents_md_for.borrow_mut() = Some(key);
    }

    pub(super) fn save_preset_agents_md(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.write_editor_to_preset(cx) {
            self.show_notice(self.error_message(error), cx);
            return;
        }
        cx.notify();
    }

    fn reload_preset_editor(&mut self) {
        *self.presets_view.agents_md_for.borrow_mut() = None;
    }

    fn write_editor_to_preset(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let Some((id, path)) = self.presets_view.agents_md_for.borrow().clone() else {
            return Ok(());
        };
        let content = self.presets_view.agents_md.read(cx).value().to_string();
        if self
            .read_preset_file(&id, &path)
            .is_ok_and(|current| current == content)
        {
            return Ok(());
        }
        if path == Path::new(AGENTS_MD) {
            self.presets_view.store.set_agents_md(&id, &content)?;
        } else {
            self.presets_view.store.write_file(&id, &path, &content)?;
        }
        self.presets_view.statuses.borrow_mut().clear();
        Ok(())
    }

    pub(super) fn open_preset_install_dialog(
        &mut self,
        id: String,
        project: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let has_skills = self
            .presets_view
            .store
            .preset(&id)
            .is_ok_and(|preset| !preset.skills.is_empty());
        let global = has_skills
            && match &project {
                Some(path) => dirs::home_dir().as_ref() == Some(path),
                None => self.projects_view.global_project_view,
            };
        let home = dirs::home_dir();
        if let Some(path) = project.filter(|path| home.as_ref() != Some(path)) {
            self.projects_view.open_project = Some(path);
        } else if self.projects_view.open_project.is_none() {
            self.projects_view.open_project = self
                .model
                .library
                .config
                .project_paths()
                .into_iter()
                .find(|path| home.as_ref() != Some(path));
        }
        self.open_install_dialog(window, cx);
        self.install_flow.global = global;
        self.install_flow.preset = Some(id);
    }

    pub(super) fn install_preset(&mut self, plan: project::InstallPlan, cx: &mut Context<Self>) {
        let Some(id) = self.install_flow.preset.clone() else {
            return;
        };
        let Some(project) = (if self.install_flow.global {
            dirs::home_dir()
        } else {
            self.projects_view.open_project.clone()
        }) else {
            self.show_notice(t!("add.project_required").to_string(), cx);
            return;
        };
        let targets = crate::agent_plugins::AgentRegistry::global()
            .install_targets()
            .into_iter()
            .filter(|target| self.install_flow.selected_targets.contains(target))
            .collect::<Vec<_>>();
        let conflicts = self
            .presets_view
            .store
            .file_conflicts(&id, &project)
            .unwrap_or_default();
        if !conflicts.is_empty() {
            self.close_dialog(cx);
            self.ask_file_conflicts(
                ConflictAction::Install {
                    id,
                    project,
                    targets,
                    plan,
                },
                conflicts
                    .into_iter()
                    .map(|conflict| conflict.path)
                    .collect(),
                cx,
            );
            return;
        }
        self.finish_preset_install(id, project, targets, plan, &FileDecisions::new(), cx);
    }

    fn finish_preset_install(
        &mut self,
        id: String,
        project: PathBuf,
        targets: Vec<InstallTarget>,
        plan: project::InstallPlan,
        decisions: &FileDecisions,
        cx: &mut Context<Self>,
    ) {
        let result = self.presets_view.store.install_reviewed_with_files(
            &id,
            &project,
            &targets,
            &self.model.skills,
            plan,
            decisions,
        );
        if result.is_ok() && !self.install_flow.global {
            self.model.library.config.touch_project(&project);
            let _ = self.model.library.save();
        }
        self.projects_view.context_estimates.borrow_mut().clear();
        self.close_dialog(cx);
        self.refresh(cx);
        match result {
            Ok(report) => self.show_preset_report(&report, t!("presets.installed"), cx),
            Err(error) => self.show_notice(self.error_message(error), cx),
        }
    }

    /// Sync the preset into the projects. Projects whose AGENTS.md block was
    /// edited locally are only overwritten after confirmation.
    pub(super) fn sync_preset(
        &mut self,
        id: String,
        projects: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let (modified, clean): (Vec<_>, Vec<_>) = projects.into_iter().partition(|project| {
            self.preset_status(&id, project).is_some_and(|status| {
                status.agents_md == AgentsMdState::Modified
                    || status
                        .files
                        .iter()
                        .any(|file| file.state == FileState::Modified && !file.removed)
            })
        });
        if !clean.is_empty() {
            self.run_preset_sync(&id, &clean, false, cx);
        }
        if !modified.is_empty() {
            self.presets_view.confirmation = Some(PresetConfirmation::Overwrite(id, modified));
            self.open_dialog(DialogKind::PresetConfirm, cx);
        }
    }

    pub(super) fn run_preset_sync(
        &mut self,
        id: &str,
        projects: &[PathBuf],
        overwrite: bool,
        cx: &mut Context<Self>,
    ) {
        let conflicts = projects
            .iter()
            .flat_map(|project| {
                self.presets_view
                    .store
                    .file_conflicts(id, project)
                    .unwrap_or_default()
            })
            .map(|conflict| conflict.path)
            .collect::<std::collections::BTreeSet<_>>();
        if !conflicts.is_empty() {
            self.ask_file_conflicts(
                ConflictAction::Sync {
                    id: id.to_string(),
                    projects: projects.to_vec(),
                    overwrite,
                },
                conflicts.into_iter().collect(),
                cx,
            );
            return;
        }
        self.finish_preset_sync(id, projects, overwrite, None, cx);
    }

    fn finish_preset_sync(
        &mut self,
        id: &str,
        projects: &[PathBuf],
        overwrite: bool,
        decision: Option<FileConflictDecision>,
        cx: &mut Context<Self>,
    ) {
        let mut report = SyncReport::default();
        let mut error = None;
        for project in projects {
            let decisions = decision
                .map(|decision| self.conflict_decisions(id, project, decision))
                .unwrap_or_default();
            match self.presets_view.store.sync_with_files(
                id,
                project,
                &self.model.skills,
                overwrite,
                &decisions,
            ) {
                Ok(partial) => {
                    report.installed += partial.installed;
                    report.removed += partial.removed;
                    report.failures.extend(partial.failures);
                }
                Err(failure) => error = Some(failure),
            }
        }
        self.projects_view.context_estimates.borrow_mut().clear();
        self.refresh(cx);
        match error {
            Some(error) => self.show_notice(self.error_message(error), cx),
            None => self.show_preset_report(&report, t!("presets.synced"), cx),
        }
    }

    fn ask_file_conflicts(
        &mut self,
        action: ConflictAction,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.presets_view.pending_conflict = Some(action);
        self.presets_view.confirmation = Some(PresetConfirmation::Conflicts(paths));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    fn conflict_decisions(
        &self,
        id: &str,
        project: &Path,
        decision: FileConflictDecision,
    ) -> FileDecisions {
        self.presets_view
            .store
            .file_conflicts(id, project)
            .unwrap_or_default()
            .into_iter()
            .map(|conflict| (conflict.path, decision))
            .collect()
    }

    pub(super) fn resolve_file_conflicts(
        &mut self,
        decision: FileConflictDecision,
        cx: &mut Context<Self>,
    ) {
        self.presets_view.confirmation = None;
        self.close_dialog(cx);
        match self.presets_view.pending_conflict.take() {
            Some(ConflictAction::Install {
                id,
                project,
                targets,
                plan,
            }) => {
                let decisions = self.conflict_decisions(&id, &project, decision);
                self.finish_preset_install(id, project, targets, plan, &decisions, cx);
            }
            Some(ConflictAction::Sync {
                id,
                projects,
                overwrite,
            }) => self.finish_preset_sync(&id, &projects, overwrite, Some(decision), cx),
            None => {}
        }
    }

    pub(super) fn request_preset_file_delete(
        &mut self,
        id: String,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.presets_view.confirmation = Some(PresetConfirmation::DeleteFile(id, path));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    pub(super) fn request_preset_uninstall(
        &mut self,
        id: String,
        project: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.presets_view.confirmation = Some(PresetConfirmation::Uninstall(id, project));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    pub(super) fn request_preset_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.presets_view.confirmation = Some(PresetConfirmation::Delete(id));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    pub(super) fn confirm_preset_action(&mut self, cx: &mut Context<Self>) {
        let Some(confirmation) = self.presets_view.confirmation.take() else {
            return;
        };
        self.close_dialog(cx);
        let skills = self.model.skills.clone();
        let (result, success) = match confirmation {
            PresetConfirmation::Delete(id) => (
                self.presets_view.store.delete(&id, &skills),
                t!("presets.deleted"),
            ),
            PresetConfirmation::Uninstall(id, project) => (
                self.presets_view.store.uninstall(&id, &project, &skills),
                t!("presets.removed"),
            ),
            PresetConfirmation::Overwrite(id, projects) => {
                self.run_preset_sync(&id, &projects, true, cx);
                return;
            }
            PresetConfirmation::DeleteFile(id, path) => {
                self.reload_preset_editor();
                if let Err(error) = self.presets_view.store.delete_file(&id, &path) {
                    self.show_notice(self.error_message(error), cx);
                }
                if self.presets_view.selected_file == path {
                    self.presets_view.selected_file = PathBuf::from(AGENTS_MD);
                }
                self.presets_view.statuses.borrow_mut().clear();
                cx.notify();
                return;
            }
            PresetConfirmation::Conflicts(_) => {
                self.resolve_file_conflicts(FileConflictDecision::Overwrite, cx);
                return;
            }
        };
        self.projects_view.context_estimates.borrow_mut().clear();
        self.refresh(cx);
        match result {
            Ok(report) => self.show_preset_report(&report, success, cx),
            Err(error) => self.show_notice(self.error_message(error), cx),
        }
    }

    fn show_preset_report(
        &mut self,
        report: &SyncReport,
        success: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        if report.failures.is_empty() {
            self.show_notice(success, cx);
            return;
        }
        for failure in &report.failures {
            eprintln!("Kitter: {failure}");
        }
        let summary = crate::i18n::tn(
            "presets.skills_failed",
            report.failures.len(),
            &[("count", report.failures.len().to_string())],
        );
        let names = report
            .failures
            .iter()
            .map(|failure| failure.skill.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.show_notice(format!("{summary}\n{names}"), cx);
    }
}
