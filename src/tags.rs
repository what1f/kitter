use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::config;

pub type TagId = u64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
    pub parent: Option<TagId>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TagState {
    #[serde(default)]
    next_id: TagId,
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default)]
    assignments: BTreeMap<String, BTreeSet<TagId>>,
}

#[derive(Default, Serialize, Deserialize)]
struct TagDocument {
    #[serde(default)]
    skills: TagState,
    #[serde(default)]
    projects: TagState,
}

pub fn load_tag_states() -> (TagState, TagState) {
    load_tag_states_from(&config::app_data_dir())
}

pub fn load_tag_states_from(data_dir: &std::path::Path) -> (TagState, TagState) {
    let path = data_dir.join("tags.json");
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<TagDocument>(&bytes).ok())
        .map(|document| (document.skills, document.projects))
        .unwrap_or_default()
}

pub fn save_tag_states(skills: &TagState, projects: &TagState) -> anyhow::Result<()> {
    save_tag_states_to(&config::app_data_dir(), skills, projects)
}

pub fn save_tag_states_to(
    data_dir: &std::path::Path,
    skills: &TagState,
    projects: &TagState,
) -> anyhow::Result<()> {
    config::save_json(
        &data_dir.join("tags.json"),
        &TagDocument {
            skills: skills.clone(),
            projects: projects.clone(),
        },
    )
}

impl TagState {
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    pub fn tag(&self, id: TagId) -> Option<&Tag> {
        self.tags.iter().find(|tag| tag.id == id)
    }

    pub fn roots(&self) -> impl Iterator<Item = &Tag> {
        self.tags.iter().filter(|tag| tag.parent.is_none())
    }

    pub fn children(&self, parent: TagId) -> impl Iterator<Item = &Tag> {
        self.tags
            .iter()
            .filter(move |tag| tag.parent == Some(parent))
    }

    pub fn add(&mut self, name: &str, parent: Option<TagId>) -> Result<TagId, &'static str> {
        let name = normalize_name(name)?;
        if let Some(parent_id) = parent {
            let Some(parent_tag) = self.tag(parent_id) else {
                return Err("父标签不存在");
            };
            if parent_tag.parent.is_some() {
                return Err("子标签下不能继续创建标签");
            }
        }
        if self.sibling_name_exists(&name, parent, None) {
            return Err("同一级已经存在这个标签");
        }
        self.next_id += 1;
        let id = self.next_id;
        self.tags.push(Tag { id, name, parent });
        Ok(id)
    }

    pub fn rename(&mut self, id: TagId, name: &str) -> Result<(), &'static str> {
        let name = normalize_name(name)?;
        let Some(tag) = self.tag(id) else {
            return Err("标签不存在");
        };
        let parent = tag.parent;
        if self.sibling_name_exists(&name, parent, Some(id)) {
            return Err("同一级已经存在这个标签");
        }
        if let Some(tag) = self.tags.iter_mut().find(|tag| tag.id == id) {
            tag.name = name;
        }
        Ok(())
    }

    pub fn delete(&mut self, id: TagId) {
        let mut removed = BTreeSet::from([id]);
        for child in self.children(id) {
            removed.insert(child.id);
        }
        self.tags.retain(|tag| !removed.contains(&tag.id));
        for tags in self.assignments.values_mut() {
            tags.retain(|tag_id| !removed.contains(tag_id));
        }
    }

    pub fn move_before(&mut self, id: TagId, target_id: TagId) -> bool {
        self.move_relative(id, target_id, false)
    }

    pub fn move_after(&mut self, id: TagId, target_id: TagId) -> bool {
        self.move_relative(id, target_id, true)
    }

    fn move_relative(&mut self, id: TagId, target_id: TagId, after: bool) -> bool {
        if id == target_id {
            return false;
        }
        let Some(source_parent) = self.tag(id).map(|tag| tag.parent) else {
            return false;
        };
        let Some(target_parent) = self.tag(target_id).map(|tag| tag.parent) else {
            return false;
        };
        if source_parent != target_parent {
            return false;
        }
        let Some(source_index) = self.tags.iter().position(|tag| tag.id == id) else {
            return false;
        };
        let Some(target_index) = self.tags.iter().position(|tag| tag.id == target_id) else {
            return false;
        };
        let tag = self.tags.remove(source_index);
        let target_index = target_index.saturating_sub(usize::from(source_index < target_index));
        let insertion_index = target_index + usize::from(after);
        self.tags.insert(insertion_index.min(self.tags.len()), tag);
        true
    }

    pub fn toggle_assignment(&mut self, skill: &str, tag_id: TagId) {
        let assigned = !self.is_assigned(skill, tag_id);
        self.set_assignment(skill, tag_id, assigned);
    }

    pub fn set_assignment(&mut self, skill: &str, tag_id: TagId, assigned: bool) -> bool {
        let Some(tag) = self.tag(tag_id) else {
            return false;
        };
        let parent = tag.parent;
        let children = if parent.is_none() {
            self.children(tag_id)
                .map(|child| child.id)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let tags = self.assignments.entry(skill.to_string()).or_default();
        let changed = if assigned {
            let changed = tags.insert(tag_id);
            if let Some(parent) = parent {
                tags.remove(&parent);
            } else {
                for child in children {
                    tags.remove(&child);
                }
            }
            changed
        } else {
            tags.remove(&tag_id)
        };
        if tags.is_empty() {
            self.assignments.remove(skill);
        }
        changed
    }

    pub fn is_assigned(&self, skill: &str, tag_id: TagId) -> bool {
        self.assignments
            .get(skill)
            .is_some_and(|tags| tags.contains(&tag_id))
    }

    pub fn assigned_tags(&self, skill: &str) -> Vec<&Tag> {
        let Some(assigned) = self.assignments.get(skill) else {
            return Vec::new();
        };
        self.tags
            .iter()
            .filter(|tag| assigned.contains(&tag.id))
            .collect()
    }

    pub fn path(&self, id: TagId) -> Option<String> {
        let tag = self.tag(id)?;
        match tag.parent.and_then(|parent| self.tag(parent)) {
            Some(parent) => Some(format!("{}/{}", parent.name, tag.name)),
            None => Some(tag.name.clone()),
        }
    }

    pub fn find_path(&self, path: &str) -> Option<TagId> {
        let path = path.trim().trim_start_matches('#').trim();
        self.tags
            .iter()
            .find(|tag| {
                self.path(tag.id)
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(path))
            })
            .map(|tag| tag.id)
    }

    pub fn count(&self, id: TagId) -> usize {
        let descendants = self
            .tag(id)
            .filter(|tag| tag.parent.is_none())
            .map(|_| {
                self.children(id)
                    .map(|child| child.id)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        self.assignments
            .values()
            .filter(|assigned| {
                assigned.contains(&id) || assigned.iter().any(|tag| descendants.contains(tag))
            })
            .count()
    }

    pub fn matches_filter(&self, skill: &str, filter: TagId) -> bool {
        let Some(assigned) = self.assignments.get(skill) else {
            return false;
        };
        if assigned.contains(&filter) {
            return true;
        }
        self.tag(filter).is_some_and(|tag| {
            tag.parent.is_none()
                && self
                    .children(filter)
                    .any(|child| assigned.contains(&child.id))
        })
    }

    fn sibling_name_exists(
        &self,
        name: &str,
        parent: Option<TagId>,
        except: Option<TagId>,
    ) -> bool {
        self.tags.iter().any(|tag| {
            tag.id != except.unwrap_or_default()
                && tag.parent == parent
                && tag.name.eq_ignore_ascii_case(name)
        })
    }
}

fn normalize_name(name: &str) -> Result<String, &'static str> {
    let name = name.trim().trim_start_matches('#').trim();
    if name.is_empty() {
        return Err("请输入标签名称");
    }
    if name.contains('/') {
        return Err("标签名称不能包含 / ");
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_two_levels_and_builds_paths() {
        let mut tags = TagState::default();
        let parent = tags.add("开发", None).unwrap();
        let child = tags.add("调试", Some(parent)).unwrap();
        assert_eq!(tags.path(child).as_deref(), Some("开发/调试"));
        assert_eq!(
            tags.add("更深", Some(child)),
            Err("子标签下不能继续创建标签")
        );
    }

    #[test]
    fn parent_filter_includes_child_assignments() {
        let mut tags = TagState::default();
        let parent = tags.add("开发", None).unwrap();
        let child = tags.add("调试", Some(parent)).unwrap();
        tags.toggle_assignment("grilling", child);
        assert!(tags.matches_filter("grilling", parent));
        assert!(tags.matches_filter("grilling", child));
        assert_eq!(tags.count(parent), 1);
    }

    #[test]
    fn assignment_can_be_set_idempotently() {
        let mut tags = TagState::default();
        let tag = tags.add("开发", None).unwrap();
        assert!(tags.set_assignment("grilling", tag, true));
        assert!(!tags.set_assignment("grilling", tag, true));
        assert!(tags.is_assigned("grilling", tag));
        assert!(tags.set_assignment("grilling", tag, false));
        assert!(!tags.set_assignment("grilling", tag, false));
        assert!(!tags.is_assigned("grilling", tag));
    }

    #[test]
    fn deleting_parent_removes_children_and_assignments() {
        let mut tags = TagState::default();
        let parent = tags.add("开发", None).unwrap();
        let child = tags.add("调试", Some(parent)).unwrap();
        tags.toggle_assignment("grilling", child);
        tags.delete(parent);
        assert!(tags.tags().is_empty());
        assert!(tags.assigned_tags("grilling").is_empty());
    }

    #[test]
    fn moves_before_target_without_crossing_levels() {
        let mut tags = TagState::default();
        let first = tags.add("开发", None).unwrap();
        let second = tags.add("研究", None).unwrap();
        let third = tags.add("设计", None).unwrap();
        let child = tags.add("调试", Some(first)).unwrap();

        assert!(tags.move_before(third, first));
        assert_eq!(
            tags.roots().map(|tag| tag.id).collect::<Vec<_>>(),
            [third, first, second]
        );
        assert!(tags.move_after(second, third));
        assert_eq!(
            tags.roots().map(|tag| tag.id).collect::<Vec<_>>(),
            [third, second, first]
        );
        assert!(!tags.move_before(child, first));
    }
}
