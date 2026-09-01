use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use gpui::{Entity, FocusHandle, ListState, ScrollHandle};
use gpui_base::TextSelectionHandle;
use gpui_component::{input::InputState, resizable::ResizableState, select::SelectState};

use crate::{
    InstallTarget, ProjectSkill, SkillLibrary, SkillSummary, adoption, source, tags::TagState,
};

use super::{
    AddKind, AddTask, ContentSnapshot, ContextEstimateCache, DeleteConfirmation, DialogBody,
    GroupEdit, Page, PageBody, ProjectChoice, ProjectSkillsTab, SourceChoice, SpinnerView,
    TagAssignmentTarget, TagDropTarget, TagEdit, TagId, TagScope, adoption_list,
    effective_skills::AgentKind, skill_selection::SkillSelection,
};

pub(super) struct AppModel {
    pub library: SkillLibrary,
    pub skills: Vec<SkillSummary>,
    pub checking_updates: bool,
    pub updating_skill: Option<String>,
}

pub(super) struct ShellState {
    pub page: Page,
    pub focus_handle: FocusHandle,
    pub header_drag_armed: bool,
    pub shell_layout: Entity<ResizableState>,
    pub content_layout: Entity<ResizableState>,
    pub spinner_accent: Entity<SpinnerView>,
    pub spinner_on_accent: Entity<SpinnerView>,
    pub notice: Option<String>,
    pub notice_generation: u64,
    pub system_dark: bool,
    pub dialog_body: Option<Entity<DialogBody>>,
    pub page_body: Entity<PageBody>,
}

pub(super) struct SkillsState {
    pub expanded_description: Option<String>,
    pub tab: super::DetailTab,
    pub selection: SkillSelection,
    pub selected_file: PathBuf,
    pub skill_search: Entity<InputState>,
    pub content_snapshot: RefCell<Option<ContentSnapshot>>,
    pub content_scroll: ScrollHandle,
    pub selectable_text_handles: RefCell<BTreeMap<String, TextSelectionHandle>>,
    pub collapsed_groups: HashSet<String>,
    pub collapsed_content_directories: HashSet<PathBuf>,
}

pub(super) struct ProjectsState {
    pub open_project: Option<PathBuf>,
    pub global_project_view: bool,
    pub project_skills_tab: ProjectSkillsTab,
    pub selected_project_agent: Option<AgentKind>,
    pub project_agents_expanded: bool,
    pub expanded_project_plugins: HashSet<String>,
    pub project_search: Entity<InputState>,
    pub project_select: Entity<SelectState<Vec<ProjectChoice>>>,
    pub project_snapshots: RefCell<BTreeMap<PathBuf, Vec<ProjectSkill>>>,
    pub context_estimates: RefCell<BTreeMap<PathBuf, ContextEstimateCache>>,
}

pub(super) struct AddFlowState {
    pub kind: AddKind,
    pub task: Option<AddTask>,
    pub scan: Option<source::SkillScan>,
    pub adoption_scan: Option<Arc<adoption::AdoptionScan>>,
    pub adoption_rows: Vec<adoption_list::AdoptionRow>,
    pub adoption_list: ListState,
    pub adoption_cancel: Option<Arc<AtomicBool>>,
    pub adoption_expanded: HashSet<String>,
    pub adoption_root: Option<PathBuf>,
    pub selected: HashSet<String>,
    pub error: Option<String>,
    pub group_enabled: bool,
    pub group_name: Option<String>,
    pub primary_input: Entity<InputState>,
    pub source_select: Entity<SelectState<Vec<SourceChoice>>>,
}

pub(super) struct InstallFlowState {
    pub modal: bool,
    pub global: bool,
    pub selected_targets: HashSet<InstallTarget>,
}

pub(super) struct DeleteFlowState {
    pub confirmation: Option<DeleteConfirmation>,
    pub selected: HashSet<PathBuf>,
    pub busy: bool,
}

pub(super) struct TagsFlowState {
    pub name_input: Entity<InputState>,
    pub skills: TagState,
    pub projects: TagState,
    pub selected_skill_filter: Option<TagId>,
    pub selected_project_filter: Option<TagId>,
    pub scope: TagScope,
    pub drop_target: Option<TagDropTarget>,
    pub edit: Option<TagEdit>,
    pub delete_pending: Option<TagId>,
    pub error: Option<String>,
    pub assignment_keys: Vec<String>,
    pub assignment_label: Option<String>,
    pub return_to_assignment: Option<TagAssignmentTarget>,
}

pub(super) struct GroupsFlowState {
    pub name_input: Entity<InputState>,
    pub edit: Option<GroupEdit>,
    pub delete_pending: Option<String>,
    pub delete_skills: bool,
    pub move_skills: Vec<String>,
}
