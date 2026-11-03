# Changelog

## v0.2.4 — 2026-11-04

- Added themes. Pick a style in Settings, including the new Solid Glass style, or build your own theme package with colors, surfaces, fonts, sizes, and icons.
- Added `kitter theme` for listing, creating, checking, and switching themes.
- Added a data location setting and `kitter data`. Move skills, presets, and settings to another folder, or switch to Kitter data copied from elsewhere.
- Skills from skills.sh and GitHub can now be added and updated with Git when Node.js is not installed.
- The Kitter skill now installs the Windows CLI to `%USERPROFILE%\.local\bin`.
- Removed `kitter library --set`. Use `kitter data --set` instead.

Update the desktop app and the CLI together. After you move your data, older versions cannot find it.

[Full changelog](https://github.com/what1f/kitter/compare/v0.2.3...v0.2.4)

## v0.2.3 — 2026-10-27

- Added copy mode for local skills, with update detection from the original folder.
- Added editable skill descriptions that leave SKILL.md untouched.
- Added instruction files to presets, with a file tree, nested folders, and conflict handling on install.
- Added scrollbars to long lists, file trees, and previews.
- Added `kitter add local --copy`, `kitter describe`, and `kitter preset file` commands.
- Improved startup speed with large skill libraries.
- Closing the window now quits Kitter on macOS.

[Full changelog](https://github.com/what1f/kitter/compare/v0.2.2...v0.2.3)

## v0.2.2 — 2026-10-23

- Added batch skill updates with progress and an Update all action.
- Added skill counts, filtered counts, and agent name tooltips.
- Improved agent skill and plugin discovery and compatibility badges.
- Improved automatic update checks and error feedback.
- Fixed false update notifications for Windows line endings and symbolic links.
- Fixed private repository imports and updates using SSH URLs.
- Fixed duplicate skill entries and incorrect installation counts.
- Preserved trigger settings when restoring missing skills.

[Full changelog](https://github.com/what1f/kitter/compare/v0.2.1...v0.2.2)

## v0.2.1 — 2026-10-22

- Fixed skill imports and unwanted console windows on Windows.
- Improved reinstall and overwrite confirmation in the desktop app and CLI.
- Improved update detection performance and UI/UX.
- Added skill version display when available.

[Full changelog](https://github.com/what1f/kitter/compare/v0.2.0...v0.2.1)
