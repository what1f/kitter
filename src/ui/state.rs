use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use gpui::{Entity, FocusHandle, ListState, ScrollHandle, UniformListScrollHandle};
use gpui_base::TextSelectionHandle;
use gpui_component::{
    input::{EditorState, InputState},
    resizable::ResizableState,
    select::SelectState,
};

use crate::{
    InstallTarget, SkillLibrary, SkillSummary, adoption,
    preset::{PresetStatus, PresetStore},
    source,
    tags::TagState,
};

use super::{
    AddKind, AddTask, ContentSnapshot, ContextEstimateCache, DeleteConfirmation, DialogBody,
    GroupDragScope, GroupEdit, Page, PageBody, PresetConfirmation, PresetEdit, PresetTab,
    ProjectChoice, ProjectSkillFilter, ProjectSkillsTab, SourceChoice, SpinnerView,
    TagAssignmentTarget, TagDropPosition, TagDropTarget, TagEdit, TagId, TagScope, adoption_list,
    effective_skills::AgentId, skill_selection::SkillSelection,
};

pub(super) struct AppModel {
    pub library: SkillLibrary,
    pub skills: Vec<SkillSummary>,
    pub update_check: Option<source::UpdateCheckProgress>,
    pub updating_skill: Option<String>,
    pub updating_batch: Option<source::UpdateProgress>,
    pub pending_updates: Option<SkillUpdateRequest>,
}

pub(super) enum SkillUpdateRequest {
    All,
    Selection(Vec<String>),
}

pub(super) struct ShellState {
    pub page: Page,
    pub focus_handle: FocusHandle,
    pub header_drag_armed: bool,
    pub shell_layout: Entity<ResizableState>,
    pub content_layout: Entity<ResizableState>,
    pub spinner_accent: Entity<SpinnerView>,
    pub spinner_update: Entity<SpinnerView>,
    pub spinner_on_accent: Entity<SpinnerView>,
    pub notice: Option<String>,
    pub notice_generation: u64,
    pub system_dark: bool,
    pub dialog_body: Option<Entity<DialogBody>>,
    /// Set while the dialog body renders, so `palette()` hands out the
    /// dialog's surfaces.
    pub painting_dialog: bool,
    pub page_body: Entity<PageBody>,
    pub data_location: Option<DataLocationPrompt>,
}

/// A chosen data folder waiting for confirmation, or being moved into.
pub(super) struct DataLocationPrompt {
    pub target: PathBuf,
    pub kind: crate::data_location::DataFolder,
    pub moving: bool,
}

pub(super) struct SkillsState {
    pub expanded_description: Option<String>,
    /// Storage name of the skill whose description is being edited.
    pub editing_description: Option<String>,
    pub description_input: Entity<gpui_component::input::TextareaState>,
    pub tab: super::DetailTab,
    pub selection: SkillSelection,
    pub selected_file: PathBuf,
    pub skill_search: Entity<InputState>,
    pub content_snapshot: RefCell<Option<ContentSnapshot>>,
    pub content_scroll: ScrollHandle,
    pub content_tree_scroll: ScrollHandle,
    pub list: ListState,
    pub list_rows: RefCell<Arc<[super::skills_page::SkillListRow]>>,
    pub selectable_text_handles: RefCell<BTreeMap<String, TextSelectionHandle>>,
    pub collapsed_groups: HashSet<String>,
    /// Groups collapsed while a search or tag filter is active, keyed by that filter.
    pub filter_collapsed_groups: (String, HashSet<String>),
    pub known_groups: HashSet<String>,
    pub collapsed_content_directories: HashSet<PathBuf>,
}

pub(super) struct ProjectsState {
    pub open_project: Option<PathBuf>,
    pub global_project_view: bool,
    pub batch_project: Option<PathBuf>,
    pub batch_selected: HashSet<String>,
    pub batch_filter: Option<ProjectSkillFilter>,
    pub project_skills_tab: ProjectSkillsTab,
    pub selected_project_agent: Option<AgentId>,
    pub project_agents_expanded: bool,
    pub expanded_project_plugins: HashSet<String>,
    pub project_search: Entity<InputState>,
    pub project_select: Entity<SelectState<Vec<ProjectChoice>>>,
    pub project_snapshots: RefCell<BTreeMap<PathBuf, Arc<super::ProjectSnapshot>>>,
    pub project_snapshot_tasks: RefCell<BTreeMap<PathBuf, u64>>,
    pub context_estimates: RefCell<BTreeMap<PathBuf, ContextEstimateCache>>,
    pub context_estimate_tasks: RefCell<BTreeMap<PathBuf, u64>>,
    pub scan_generation: u64,
    pub list_scroll: ScrollHandle,
    pub skills_scroll: UniformListScrollHandle,
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
    /// A single skill joins no group unless the user picks one.
    pub group_chosen: bool,
    pub local_mode: crate::LocalInstallMode,
    pub primary_input: Entity<InputState>,
    pub source_select: Entity<SelectState<Vec<SourceChoice>>>,
}

pub(super) struct InstallFlowState {
    pub modal: bool,
    pub global: bool,
    pub selected_targets: HashSet<InstallTarget>,
    pub pending: Option<crate::project::InstallPlan>,
    pub error: Option<String>,
    /// Set when the dialog installs a preset instead of the selected skills.
    pub preset: Option<String>,
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
    pub drop_target: Option<(GroupDragScope, String, TagDropPosition)>,
    pub name_input: Entity<InputState>,
    pub edit: Option<GroupEdit>,
    pub delete_pending: Option<String>,
    pub delete_skills: bool,
    pub move_skills: Vec<String>,
}

pub(super) struct PresetsState {
    pub store: PresetStore,
    pub selected: Option<String>,
    pub tab: PresetTab,
    pub name_input: Entity<InputState>,
    pub edit: Option<PresetEdit>,
    pub error: Option<String>,
    pub agents_md: Entity<EditorState>,
    /// The preset file currently loaded into the editor.
    pub agents_md_for: RefCell<Option<(String, PathBuf)>>,
    pub selected_file: PathBuf,
    pub collapsed_dirs: HashSet<PathBuf>,
    pub tree_scroll: ScrollHandle,
    /// Install or sync waiting for the user to resolve file conflicts.
    pub pending_conflict: Option<super::preset_actions::ConflictAction>,
    pub skill_search: Entity<InputState>,
    pub picked_skills: HashSet<String>,
    pub confirmation: Option<PresetConfirmation>,
    pub statuses: RefCell<BTreeMap<(String, PathBuf), Option<PresetStatus>>>,
    pub list_scroll: ScrollHandle,
}
