use super::*;

impl KitterApp {
    pub(super) fn open_tag_dialog(&mut self, scope: TagScope, cx: &mut Context<Self>) {
        self.tags_flow.scope = scope;
        self.tags_flow.assignment_keys.clear();
        self.tags_flow.assignment_label = None;
        self.tags_flow.return_to_assignment = None;
        self.tags_flow.drop_target = None;
        self.tags_flow.edit = None;
        self.tags_flow.delete_pending = None;
        self.tags_flow.error = None;
        self.open_dialog(DialogKind::Tags, cx);
    }

    pub(super) fn open_tag_creation_from_assignment(&mut self, cx: &mut Context<Self>) {
        if self.tags_flow.assignment_keys.is_empty() {
            return;
        }
        let target = TagAssignmentTarget {
            scope: self.tags_flow.scope,
            keys: self.tags_flow.assignment_keys.clone(),
            label: self
                .tags_flow
                .assignment_label
                .clone()
                .unwrap_or_else(|| self.tags_flow.assignment_keys.join(", ")),
        };
        let scope = target.scope;
        self.open_tag_dialog(scope, cx);
        self.tags_flow.return_to_assignment = Some(target);
        self.notify_dialog(cx);
    }

    pub(super) fn open_tag_assignment_target(
        &mut self,
        target: TagAssignmentTarget,
        cx: &mut Context<Self>,
    ) {
        if target.scope == TagScope::Skills {
            let primary = self.skills_view.selection.replace(&target.keys);
            self.set_detail_selection(primary);
        }
        self.tags_flow.scope = target.scope;
        self.tags_flow.assignment_keys = target.keys;
        self.tags_flow.assignment_label = Some(target.label);
        self.open_dialog(DialogKind::AssignTags, cx);
    }

    pub(super) fn open_group_dialog(&mut self, cx: &mut Context<Self>) {
        self.groups_flow.edit = None;
        self.groups_flow.delete_pending = None;
        self.tags_flow.error = None;
        self.open_dialog(DialogKind::Groups, cx);
    }

    pub(super) fn open_group_delete_dialog(&mut self, id: String, cx: &mut Context<Self>) {
        self.groups_flow.edit = None;
        self.groups_flow.delete_pending = Some(id);
        self.groups_flow.delete_skills = false;
        self.tags_flow.error = None;
        self.open_dialog(DialogKind::DeleteGroup, cx);
    }

    pub(super) fn open_move_group_dialog_for_selection(
        &mut self,
        skills: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.groups_flow.move_skills = skills;
        self.open_dialog(DialogKind::MoveGroup, cx);
    }

    pub(super) fn open_tag_assignment_dialog_for_selection(
        &mut self,
        skills: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(first) = skills.first().cloned() else {
            return;
        };
        self.tags_flow.return_to_assignment = None;
        let primary = self.skills_view.selection.replace(&skills);
        self.set_detail_selection(primary);
        self.tags_flow.scope = TagScope::Skills;
        self.tags_flow.assignment_keys = skills.clone();
        self.tags_flow.assignment_label = if skills.len() == 1 {
            self.selected_skill()
                .map(|skill| skill.record.name.clone())
                .or_else(|| Some(first))
        } else if self.uses_english() {
            Some(format!("{} Skills", skills.len()))
        } else {
            Some(format!("{} 个技能", skills.len()))
        };
        self.open_dialog(DialogKind::AssignTags, cx);
    }

    pub(super) fn open_project_tag_assignment_dialog(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.tags_flow.return_to_assignment = None;
        self.tags_flow.scope = TagScope::Projects;
        self.tags_flow.assignment_keys = vec![project_tag_key(&path)];
        self.tags_flow.assignment_label = Some(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        );
        self.open_dialog(DialogKind::AssignTags, cx);
    }

    pub(super) fn start_tag_edit(
        &mut self,
        edit: TagEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scope = self.tags_flow.scope;
        let value = match edit {
            TagEdit::Rename(id) => self
                .tags_for(scope)
                .tag(id)
                .map(|tag| tag.name.clone())
                .unwrap_or_default(),
            TagEdit::CreateRoot | TagEdit::CreateChild(_) => String::new(),
        };
        self.tags_flow.name_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
        self.tags_flow.edit = Some(edit);
        self.tags_flow.delete_pending = None;
        self.tags_flow.error = None;
        self.notify_dialog(cx);
    }

    pub(super) fn commit_tag_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.tags_flow.edit else {
            return;
        };
        let name = self.tags_flow.name_input.read(cx).value().to_string();
        let scope = self.tags_flow.scope;
        let result = match edit {
            TagEdit::CreateRoot => self.tags_for_mut(scope).add(&name, None).map(|_| ()),
            TagEdit::CreateChild(parent) => self
                .tags_for_mut(scope)
                .add(&name, Some(parent))
                .map(|_| ()),
            TagEdit::Rename(id) => self.tags_for_mut(scope).rename(id, &name),
        };
        match result {
            Ok(()) => {
                self.tags_flow.edit = None;
                self.tags_flow.error = None;
                self.persist_tags();
                if let Some(target) = self.tags_flow.return_to_assignment.take() {
                    self.open_tag_assignment_target(target, cx);
                    return;
                }
            }
            Err(error) => self.tags_flow.error = Some(error.to_string()),
        }
        self.notify_dialog(cx);
        cx.notify();
    }

    pub(super) fn start_group_edit(
        &mut self,
        edit: GroupEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = match &edit {
            GroupEdit::Create => String::new(),
            GroupEdit::Rename(id) => self
                .model
                .library
                .groups()
                .into_iter()
                .find(|group| &group.id == id)
                .map(|group| group.name)
                .unwrap_or_default(),
        };
        self.groups_flow.name_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
        self.groups_flow.edit = Some(edit);
        self.groups_flow.delete_pending = None;
        self.tags_flow.error = None;
        self.notify_dialog(cx);
    }

    pub(super) fn commit_group_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.groups_flow.edit.clone() else {
            return;
        };
        let name = self
            .groups_flow
            .name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let result = match edit {
            GroupEdit::Create => self.model.library.create_group(&name).map(|_| ()),
            GroupEdit::Rename(id) => self.model.library.rename_group(&id, &name),
        };
        match result {
            Ok(()) => {
                self.groups_flow.edit = None;
                self.tags_flow.error = None;
                self.refresh(cx);
            }
            Err(error) => self.tags_flow.error = Some(error.to_string()),
        }
        self.notify_dialog(cx);
        cx.notify();
    }

    pub(super) fn assign_skill_group(
        &mut self,
        skill: String,
        group_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match self
            .model
            .library
            .assign_group_by_storage(&skill, group_id.as_deref())
        {
            Ok(()) => {
                self.refresh(cx);
                self.show_notice(self.tr("已更新技能分组", "Skill group updated"), cx);
            }
            Err(error) => self.show_notice(error.to_string(), cx),
        }
    }

    pub(super) fn move_selected_skill_to_group(
        &mut self,
        group_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.groups_flow.move_skills.is_empty() {
            self.close_dialog(cx);
            return;
        }
        let skills = self.groups_flow.move_skills.clone();
        let mut failure = None;
        for skill in skills {
            if let Err(error) = self
                .model
                .library
                .assign_group_by_storage(&skill, group_id.as_deref())
            {
                failure = Some(error.to_string());
                break;
            }
        }
        match failure {
            None => {
                self.groups_flow.move_skills.clear();
                self.close_dialog(cx);
                self.refresh(cx);
                self.show_notice(self.tr("已更新技能分组", "Skill group updated"), cx);
            }
            Some(error) => self.show_notice(error, cx),
        }
    }

    pub(super) fn delete_skill_group(
        &mut self,
        id: String,
        delete_skills: bool,
        cx: &mut Context<Self>,
    ) {
        match self.model.library.delete_group(&id, delete_skills) {
            Ok(names) => {
                self.close_dialog(cx);
                let order = self
                    .model
                    .skills
                    .iter()
                    .map(|skill| skill_storage_name(skill).to_string())
                    .filter(|name| !names.contains(name))
                    .collect::<Vec<_>>();
                self.skills_view.selection.remove(&names, &order);
                self.refresh(cx);
                let message = if delete_skills {
                    self.tr("已删除分组及其中的技能", "Group and its Skills deleted")
                } else {
                    self.tr(
                        "已删除分组，技能已移到未分组",
                        "Group deleted; Skills are now ungrouped",
                    )
                };
                self.show_notice(message, cx);
            }
            Err(error) => {
                self.tags_flow.error = Some(error.to_string());
                self.notify_dialog(cx);
            }
        }
    }
}
