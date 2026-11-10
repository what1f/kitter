# 更新日志

## v0.2.6 — 2026-11-10

- macOS 版现已通过 Apple 签名和公证，首次打开不再被系统拦截。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.5...v0.2.6)

## v0.2.5 — 2026-11-10

- 设置中新增「关于」，显示版本和更新日志，可以直接更新 Kitter。
- 新增 `kitter self-update`。
- 点击 Skill 的来源可以打开仓库；本地 Skill 会打开所在文件夹。
- 添加单个 Skill 时也可以选择分组。
- 预设的指令文件支持拖到指定文件夹，也可以拖动文件和文件夹来移动。
- 搜索或按标签筛选时也可以收起分组。
- 没有 Skill 的预设不能再全局安装。
- 修复 Windows 上预设的项目路径显示 `\\?\` 前缀的问题。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.4...v0.2.5)

## v0.2.4 — 2026-11-04

- 新增主题。可以在设置中选择风格，包括新的 Solid Glass；也可以用颜色、材质、字体、尺寸和图标制作自己的主题包。
- 新增 `kitter theme`，用于列出、创建、检查和切换主题。
- 新增数据位置设置和 `kitter data`。可以把 Skill、预设和设置移到另一个文件夹，或者切换到从别处复制来的 Kitter 数据。
- 没有安装 Node.js 时，也能通过 Git 添加和更新来自 skills.sh 与 GitHub 的 Skill。
- Kitter skill 现在会把 Windows 版 CLI 安装到 `%USERPROFILE%\.local\bin`。
- 把选中的 Skill 拖到分组时，会移动所有选中的 Skill。
- 修复 Windows 上预设中子文件夹里的文件安装失败的问题。
- 移除 `kitter library --set`，请改用 `kitter data --set`。

请同时更新桌面端和 CLI。移动数据后，旧版本将找不到数据。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.3...v0.2.4)

## v0.2.3 — 2026-10-27

- 本地 Skill 新增复制模式，并能检测原文件夹中的更新。
- Skill 描述可以编辑，且不会改动 SKILL.md。
- 预设支持指令文件，带文件树、多级文件夹，安装时可处理冲突。
- 长列表、文件树和预览新增滚动条。
- 新增 `kitter add local --copy`、`kitter describe` 和 `kitter preset file` 命令。
- Skill 库较大时启动更快。
- 在 macOS 上关闭窗口即退出 Kitter。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.2...v0.2.3)

## v0.2.2 — 2026-10-23

- 新增批量更新 Skill，显示进度，并提供「全部更新」。
- 新增 Skill 数量、筛选后的数量，以及 Agent 名称提示。
- 改进 Agent 的 Skill 与插件发现，以及兼容性标记。
- 改进自动检查更新和错误提示。
- 修复 Windows 换行符和符号链接导致的误报更新。
- 修复通过 SSH 地址导入和更新私有仓库。
- 修复重复的 Skill 条目和错误的安装数量。
- 恢复丢失的 Skill 时保留触发设置。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.1...v0.2.2)

## v0.2.1 — 2026-10-22

- 修复 Windows 上的 Skill 导入问题和多余的控制台窗口。
- 改进桌面端和 CLI 中重新安装与覆盖的确认。
- 改进更新检测的性能与界面体验。
- 有版本信息时显示 Skill 版本。

[完整变更](https://github.com/what1f/kitter/compare/v0.2.0...v0.2.1)
