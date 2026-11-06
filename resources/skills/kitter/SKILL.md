---
name: kitter
description: Use Kitter CLI to manage a reusable skill library, add or adopt skill sources, install them globally or per project, inspect effective skills, organize the library, bundle skills and AGENTS.md instructions into presets, maintain updates, and design desktop themes. Applies Kitter's minimal-global, project-first best practice.
disable-model-invocation: true
metadata:
  opencode:
    autoinvoke: false
---

# Kitter

Kitter manages Agent Skills on the current machine. Its core principle is simple: maintain one source for each skill, then install it only where it is needed.

## Run the CLI

The desktop app and CLI are separate release artifacts. This skill uses the standalone CLI; the desktop app does not contain or install it.

Before doing any work, resolve the executable in this order:

1. Resolve `kitter` from `PATH` (`Get-Command kitter` on Windows), and convert the result to an absolute path.
2. If it is not on `PATH`, check the supported per-user install location: `~/.local/bin/kitter` on macOS or Linux, or `%USERPROFILE%\.local\bin\kitter.exe` on Windows (older installs may use `%LOCALAPPDATA%\Kitter\bin\kitter.exe`).
3. Run the resolved executable with `--help` and confirm that it is the Kitter skill manager.

The examples below use `kitter` for readability; when executing them, always use the absolute path you resolved.

To update the CLI itself to the latest release, run `kitter self-update`; `kitter self-update --check` only reports whether a newer version exists. Ask before installing. Updating the desktop app from its Settings also updates a CLI found in the per-user install location.

If the executable does not exist, read [references/install-cli.md](references/install-cli.md). Explain that the skill needs the standalone CLI, then offer the supported installation path for the current platform. Download or install it only after the user agrees. Do not search download folders, install unrelated package managers, or modify shell profiles.

```text
Local folders / GitHub / skills.sh / existing installations
                              | add / adopt
                              v
                    Kitter skill library
                              | linked install
                 +------------+------------+
                 v                         v
          User-level agent dirs      Project agent dirs
          A few universal skills     skills that project needs
```

One skill can serve multiple agents and projects without creating independent copies. Updating a managed source updates every linked installation.

## Recommended practice

Use Kitter with a managed-library, minimal-global, project-first approach:

- Add skills that need long-term maintenance to the Kitter library.
- Install only a small set of genuinely universal skills at user scope.
- Install language, framework, business, team-process, and task-specific skills into projects.
- When several projects need the same skill, install it from Kitter instead of copying folders between projects.
- Do not reinstall a skill when an enabled plugin already provides the same capability.
- Use `project` to inspect the skills that each agent actually sees and their estimated context cost.

A useful scope test is: if the skill remains useful in almost every project, consider a user-level installation; otherwise install it in the project.

## Start with the library

Show the library location and its managed skills:

```bash
kitter library
kitter list
```

Inspect a skill's source, description, files, or instructions:

```bash
kitter show <skill>
kitter files <skill>
kitter read <skill> SKILL.md
```

Use the skill name when it is unique. If multiple skills share a name, `kitter list` displays an `id:<value>` selector; use that selector in later commands to choose the exact skill.

To customize a skill's description in Kitter:

```bash
kitter describe <skill> "My description"
kitter describe <skill> --reset
```

An empty or whitespace-only description also restores the original. `list` and `show` display the custom description when set. This does not change the skill's instructions or `SKILL.md`.

## Add skills to Kitter

### Add a new source

Add skills from a local folder:

```bash
kitter add local /path/to/skills
kitter add local /path/to/skills --skill skill-a --skill skill-b
kitter add local /path/to/skills --copy
```

Local skills are linked by default, so edits take effect immediately. Use `--copy` to keep an independent copy in Kitter. Run `kitter check` to detect source changes, then `kitter update <skill>` to copy them again. If the original folder is removed, the copy remains usable.

Add from GitHub or a skills.sh-compatible source:

```bash
kitter add npx https://github.com/owner/repository
kitter add npx https://github.com/owner/repository --skill skill-a
```

Add from a Claude plugin source:

```bash
kitter add claude <plugin>
kitter add claude <plugin> --skill skill-a
```

Without `--skill`, Kitter adds every discovered skill. Repeat `--skill` to select only specific skills. Use `--group <group>` to organize them while adding.

### Adopt existing installations

If skills already exist in user or project agent directories, scan them first:

```bash
kitter adopt /path/to/project
kitter adopt /path/to/project --json
```

The scan reports sources, references, and conflicts without changing anything. After reviewing the result, adopt every unambiguous candidate:

```bash
kitter adopt /path/to/project --all
```

When several same-named sources exist, select the source to keep:

```bash
kitter adopt /path/to/project --source /exact/source/path
```

Adopting an external source does not move it. Kitter keeps that directory as the source of truth and manages the known installation links that point to it.

## Install skills

### Install into a project

Use the shared agent directory when compatible agents in the same project should use a skill:

```bash
kitter install skill-a skill-b \
  --project /path/to/project \
  --target universal
```

Install only for one agent when needed:

```bash
kitter install skill-a \
  --project /path/to/project \
  --target codex
```

### Install at user scope

Install genuinely universal skills at user scope:

```bash
kitter install kitter \
  --project "$HOME" \
  --target universal
```

Install globally for only one agent:

```bash
kitter install <skill> \
  --project "$HOME" \
  --target codex
```

Built-in Agent ids are `codex`, `claude-code`, `cursor`, `opencode`, `pi`, `omp`, `dimagent`, `grok`, `openclaw`, `hermes`, `droid`, `amp`, `antigravity`, `copilot`, and `trae`. Only Agents with an install target accept `--target`; `universal` is the shared target. Repeat `--target` to install into several targets in one operation. Use `universal` for DimAgent; omp supports both `universal` and `omp`. The legacy alias `claude` still works; use `claude-code` in new commands. JSON `agent` and `target` fields use canonical ids such as `claude-code`.

## Agents

Run `kitter agents` to see available Agents and plugin errors. Manage them with:

```bash
kitter agents add <github-url|path> [--only <id>]
kitter agents update [<id>]
kitter agents remove <id>
kitter agents enable <id>
kitter agents disable <id>
kitter agents move <id> --before <id>
```

Disabled Agents are hidden from project inspection and cannot be install targets. When the user needs an Agent that Kitter does not support, or wants to change a built-in one, read [references/agent-plugins.md](references/agent-plugins.md).

## Desktop themes

The desktop app's look comes from a theme: the built-in `standard` and `solid_glass`, or a theme file or theme package in the themes folder. A theme sets colours, surface finish, shapes, font, sizes and icons.

```bash
kitter theme
kitter theme use <id>
kitter theme create <id> --from solid_glass
kitter theme check <id|path>
kitter theme show <id>
kitter theme icons --theme <id>
kitter theme add <file-or-folder>
kitter theme remove <id>
```

`use` takes effect after the desktop app restarts, or when the user picks the theme again in Settings → Style. When the user wants to design or adjust a theme, read [references/themes.md](references/themes.md).

## Common workflows

### Add one skill from GitHub and use it in a project

```bash
kitter add npx https://github.com/owner/repository --skill skill-a
kitter install skill-a --project /path/to/project --target universal
kitter project /path/to/project
```

### Share one skill across several projects

```bash
kitter install skill-a --project /path/to/project-a --target universal
kitter install skill-a --project /path/to/project-b --target universal
```

Both projects now link to the same managed source, so they do not need separate updates.

### Move a project-specific skill out of global scope

```bash
kitter uninstall skill-a --project "$HOME"
kitter install skill-a --project /path/to/project --target universal
```

### Adopt a skill from one project and reuse it elsewhere

```bash
kitter adopt /path/to/project-a --json
kitter adopt /path/to/project-a --source /exact/source/path
kitter install skill-a --project /path/to/project-b --target universal
```

## Inspect effective skills

Inspect user-level effective skills:

```bash
kitter project "$HOME"
```

Inspect direct installations, agent-visible skills, and context estimates for a project:

```bash
kitter project /path/to/project
kitter project /path/to/project --agent codex
```

Inspect filesystem skills or plugin-provided skills separately:

```bash
kitter project /path/to/project --view skills
kitter project /path/to/project --view plugins
kitter project /path/to/project --agent codex --view plugins --json
```

Use these results to confirm that the project has the skills it needs and to detect capabilities already supplied globally or by plugins.

## Organize the library

Groups provide a primary domain-based organization:

```bash
kitter group list
kitter group create frontend
kitter group assign frontend react typescript
kitter group clear react
kitter group rename frontend web
kitter group delete web
```

Deleting a group keeps its skills by default. Use `--delete-skills` only when the skills themselves should also be removed.

Tags support cross-cutting classification and filtering:

```bash
kitter tag list
kitter tag create testing
kitter tag create e2e --parent testing
kitter tag assign e2e playwright
kitter tag unassign e2e playwright
kitter list --tag testing
```

If tags with the same name exist under different parents, use the `id:<value>` selector shown by `kitter tag list`.

## Use presets

A preset bundles library skills with optional AGENTS.md content and text files so a project can be set up in one step. Use presets when several projects share the same stack or team conventions.

```bash
kitter preset create frontend
kitter preset add frontend react typescript
kitter preset agents-md frontend --set AGENTS.frontend.md
kitter preset install frontend --project /path/to/project --target universal
```

Installing a preset links its skills into the project and appends its AGENTS.md content to the project's `AGENTS.md` inside a managed block. Content outside that block belongs to the user and is never changed. User-scope installations (`--project "$HOME"`) omit AGENTS.md and the other instruction files.

Add text files at project-relative paths. Use UTF-8 text; set AGENTS.md with `agents-md` instead. Kitter does not interpret links or references inside AGENTS.md.

```bash
kitter preset file write frontend docs/conventions.md ./conventions.md
kitter preset file write frontend .cursor/rules/swift.mdc - < swift.mdc
kitter preset file list frontend
kitter preset file read frontend docs/conventions.md
kitter preset file rename frontend docs/conventions.md docs/team.md
kitter preset file delete frontend docs/team.md
```

If an existing project file has different content, installation stops and lists the paths. After reviewing them, choose `--overwrite-conflicts` or `--skip-conflicts`. Skipped files stay untouched by later syncs and removal. `apply` is an alias for `install`.

```bash
kitter preset install frontend --project /path/to/project --target universal --skip-conflicts
kitter preset sync frontend --project /path/to/project --overwrite-conflicts
```

Editing a preset does not change projects automatically. Inspect which projects need a sync, then apply the latest content:

```bash
kitter preset show frontend
kitter preset sync frontend
kitter preset sync frontend --project /path/to/project
```

If a preset file or the AGENTS.md preset content was edited inside a project, sync stops for that project. Confirm with the user before overwriting it with `--force`. New files can also need `--overwrite-conflicts` or `--skip-conflicts`.

When a file is deleted from the preset, sync removes the project copy if it is unchanged. Edited copies are kept and left alone on later syncs.

Remove a preset from one project, or delete it everywhere:

```bash
kitter preset uninstall frontend --project /path/to/project
kitter preset delete frontend
```

Removing a preset removes its skills, its AGENTS.md content, and unchanged copies of its text files. Edited files and pre-existing folders are kept. Empty folders created for the preset's files are removed. Skills that the project installed directly, or that another installed preset still includes, are kept.

Other commands: `kitter preset list`, `kitter preset rename <preset> <name>`, `kitter preset remove <preset> <skills>...`, `kitter preset agents-md <preset>` to print the content, and `kitter preset agents-md <preset> --clear`.

## Update skills

Check managed skills for updates:

```bash
kitter check
kitter check --json
```

Update selected skills or every managed source:

```bash
kitter update skill-a skill-b
kitter update --all
```

Projects installed through managed links continue using the updated source. Update adopted external sources with their original tool; Kitter does not overwrite them.

## Uninstall and remove skills

Uninstall a skill from one project:

```bash
kitter uninstall skill-a --project /path/to/project
```

Uninstall it only from one agent target:

```bash
kitter uninstall skill-a \
  --project /path/to/project \
  --target codex
```

Uninstall an exact installation path:

```bash
kitter uninstall \
  --project /path/to/project \
  --path .agents/skills/skill-a
```

Stop managing a skill in the Kitter library:

```bash
kitter remove skill-a
```

`uninstall` changes where a skill is installed. `remove` removes it from the Kitter library and cleans up installations that Kitter still manages.

External symlinks and real directories are preserved by default. Use `--include-unmanaged` for an exact path only after confirming that it should also be removed.

## Change the data location

Kitter keeps skills, presets, and settings in one data folder:

```bash
kitter data
kitter data --set /absolute/path/to/folder
```

- An empty folder: Kitter moves the data there and updates the skills it installed. It lists any installations it could not update; install those again.
- A folder that already holds Kitter data: Kitter switches to it and leaves the current data in place.

Confirm with the user before running `--set`, because it changes where every later command reads and writes.

For complete command options, use the relevant help command:

```bash
kitter --help
kitter install --help
kitter adopt --help
kitter preset --help
```
