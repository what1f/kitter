// Presentation of core errors belongs at the UI boundary; CLI diagnostics remain intact.
use super::KitterApp;

const MESSAGES: &[(&str, &str)] = &[
    ("请输入标签名称", "Enter a tag name"),
    ("标签名称不能包含 /", "Tag names cannot contain /"),
    ("父标签不存在", "Parent tag no longer exists"),
    (
        "子标签下不能继续创建标签",
        "Nested tags cannot contain more tags",
    ),
    (
        "同一级已经存在这个标签",
        "A tag with this name already exists here",
    ),
    ("标签不存在", "Tag no longer exists"),
    ("请输入分组名称", "Enter a group name"),
    ("已经存在同名分组", "A group with this name already exists"),
    ("分组不存在", "Group no longer exists"),
    ("技能不存在", "Skill no longer exists"),
    ("请选择技能文件夹", "Choose a skill folder"),
    ("请选择一个文件夹", "Choose a folder"),
    ("请至少选择一个技能", "Select at least one skill"),
    (
        "这个文件夹中没有找到可用的技能",
        "No skills found in this folder",
    ),
    (
        "这个地址中没有找到可用的技能",
        "No skills found at this address",
    ),
    (
        "这个插件中没有找到可用的技能",
        "No skills found in this plugin",
    ),
    (
        "请输入 skills.sh 或 GitHub 地址",
        "Enter a skills.sh or GitHub address",
    ),
    (
        "没有识别到 skills.sh 或 GitHub 地址",
        "Enter a valid skills.sh or GitHub address",
    ),
    ("请输入 Claude 插件名称", "Enter a Claude plugin name"),
    (
        "没有识别到 Claude 插件名称",
        "Enter a valid Claude plugin name",
    ),
    (
        "所选目录中没有 SKILL.md",
        "No SKILL.md found in the selected folder",
    ),
    (
        "仓库中没有找到 SKILL.md",
        "No SKILL.md found in this repository",
    ),
    (
        "Claude 插件中没有找到技能",
        "No skills found in this Claude plugin",
    ),
    (
        "Claude 插件中没有找到指定技能",
        "Skill not found in this Claude plugin",
    ),
    (
        "此技能链接到原始目录，请在来源中更新",
        "Update this skill in its original folder",
    ),
    (
        "先将技能的触发时机设为跟随技能",
        "Set trigger timing to Follow skill first",
    ),
    (
        "Kitter 内置 Skill 会随 Kitter 自动更新",
        "This built-in skill updates with Kitter",
    ),
    (
        "Kitter 内置 Skill 不能删除",
        "This built-in skill cannot be deleted",
    ),
    (
        "Kitter 内置 Skill 固定显示在列表顶部",
        "This built-in skill stays at the top of the list",
    ),
    (
        "这个技能没有可用的更新来源",
        "No update source is available for this skill",
    ),
    (
        "该文件不是可预览的文本文件",
        "This file cannot be previewed as text",
    ),
    (
        "文件不在技能目录内",
        "Choose a file inside the skill folder",
    ),
    ("技能名称无效", "Invalid skill name"),
    ("无法确定技能名称", "Could not read the skill name"),
    ("来源记录不一致", "Skill sources do not match"),
    (
        "Npx 扫描结果不可用，请重新扫描",
        "Scan the source again before adding skills",
    ),
    (
        "引用已变化，请重新扫描",
        "The skill location changed. Scan again",
    ),
    ("找不到用户目录", "Home folder could not be found"),
    ("无法确定用户目录", "Home folder could not be found"),
    ("已取消扫描", "Scan cancelled"),
];

pub(super) fn error_message(message: &str, english: bool) -> String {
    if let Some((zh, en)) = MESSAGES.iter().find(|(zh, _)| *zh == message.trim()) {
        return if english { *en } else { *zh }.into();
    }
    // Keep paths, command output, and implementation details in diagnostics.
    eprintln!("Kitter: {message}");
    let (zh, en) = if message.starts_with("技能已变化")
        || message.starts_with("引用已变化")
        || message.starts_with("引用验证失败")
    {
        (
            "技能位置或内容已变化，请重新扫描",
            "The skill changed. Scan again",
        )
    } else if message.starts_with("没有找到技能") {
        (
            "没有找到这个技能，请刷新后重试",
            "Skill not found. Refresh and try again",
        )
    } else if message.starts_with("这个来源中的技能已添加") {
        ("这个技能已经添加", "This skill has already been added")
    } else if message.starts_with("发现多个名为") {
        (
            "发现同名技能，请调整名称后重试",
            "Some skills share a name. Rename them and try again",
        )
    } else if message.starts_with("没有从来源中找到技能") {
        ("没有从这个来源中找到技能", "No skills found in this source")
    } else if message.starts_with("无法启动") {
        (
            "所需工具不可用，请确认已安装后重试",
            "A required tool is unavailable. Check that it is installed and try again",
        )
    } else if message.starts_with("命令执行失败") {
        (
            "无法获取技能，请检查来源和网络连接后重试",
            "Could not fetch skills. Check the source and your connection, then try again",
        )
    } else if message.starts_with("部分技能检查失败") {
        (
            "部分技能无法检查更新，请稍后重试",
            "Some skills could not be checked for updates. Try again later",
        )
    } else {
        (
            "操作未完成，请重试",
            "The operation could not be completed. Please try again",
        )
    };
    if english { en } else { zh }.into()
}

impl KitterApp {
    pub(super) fn source_label(&self, origin: &crate::SkillOrigin) -> String {
        match origin.source() {
            crate::SkillSource::Unknown => self.tr("本地技能", "Local skill").into(),
            crate::SkillSource::Local { ref path } if path.file_name().is_none() => {
                self.tr("本地导入", "Local import").into()
            }
            source => source.label(),
        }
    }

    pub(super) fn error_message(&self, error: impl std::fmt::Display) -> String {
        error_message(&error.to_string(), self.uses_english())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_and_source_errors_have_english_messages() {
        for (zh, en) in MESSAGES {
            assert_eq!(error_message(zh, true), *en);
            assert!(!en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
        }
        assert_eq!(
            error_message("标签名称不能包含 / ", true),
            "Tag names cannot contain /"
        );
    }

    #[test]
    fn diagnostics_do_not_leak_into_ui_errors() {
        assert_eq!(
            error_message("Kitter 内置 Skill 不能删除", true),
            "This built-in skill cannot be deleted"
        );
        let raw = "命令执行失败：internal command output /private/path";
        let message = error_message(raw, true);
        assert!(message.contains("Could not fetch skills"));
        assert!(!message.contains("/private"));
        assert!(!error_message("unknown backend failure", false).contains("backend"));
    }
}
