---
name: kitter
description: Use Kitter CLI to manage a reusable Skill library, add or adopt Skill sources, install them globally or per project, inspect effective Skills, organize the library, and maintain updates. Applies Kitter's minimal-global, project-first best practice.
disable-model-invocation: true
metadata:
  opencode:
    autoinvoke: false
---

# Kitter

Kitter manages Agent Skills on the current machine. Its core principle is simple: maintain one source for each Skill, then install it only where it is needed.

## Run the CLI

The desktop app and CLI are separate release artifacts. This Skill uses the standalone CLI; the desktop app does not contain or install it.

Before doing any work, resolve the executable in this order:

1. Resolve `kitter` from `PATH` (`Get-Command kitter` on Windows), and convert the result to an absolute path.
2. If it is not on `PATH`, check the supported per-user install location: `~/.local/bin/kitter` on macOS or Linux, or `%LOCALAPPDATA%\Kitter\bin\kitter.exe` on Windows.
3. Run the resolved executable with `--help` and confirm that it is the Kitter Skill manager.

The examples below use `kitter` for readability; when executing them, always use the absolute path you resolved.

If the executable does not exist, read [references/install-cli.md](references/install-cli.md). Explain that the Skill needs the standalone CLI, then offer the supported installation path for the current platform. Download or install it only after the user agrees. Do not search download folders, install unrelated package managers, or modify shell profiles.

```text
Local folders / GitHub / skills.sh / existing installations
                              | add / adopt
                              v
                    Kitter Skill library
                              | linked install
                 +------------+------------+
                 v                         v
          User-level Agent dirs      Project Agent dirs
          A few universal Skills     Skills that project needs
```

One Skill can serve multiple Agents and projects without creating independent copies. Updating a managed source updates every linked installation.

## Recommended practice

Use Kitter with a managed-library, minimal-global, project-first approach:

- Add Skills that need long-term maintenance to the Kitter library.
- Install only a small set of genuinely universal Skills at user scope.
- Install language, framework, business, team-process, and task-specific Skills into projects.
- When several projects need the same Skill, install it from Kitter instead of copying folders between projects.
- Do not reinstall a Skill when an enabled plugin already provides the same capability.
- Use `project` to inspect the Skills that each Agent actually sees and their estimated context cost.

A useful scope test is: if the Skill remains useful in almost every project, consider a user-level installation; otherwise install it in the project.

## Start with the library

Show the library location and its managed Skills:

```bash
kitter library
kitter list
```

Inspect a Skill's source, description, files, or instructions:

```bash
kitter show <skill>
kitter files <skill>
kitter read <skill> SKILL.md
```

Use the Skill name when it is unique. If multiple Skills share a name, `kitter list` displays an `id:<value>` selector; use that selector in later commands to choose the exact Skill.

## Add Skills to Kitter

### Add a new source

Add Skills from a local folder:

```bash
kitter add local /path/to/skills
kitter add local /path/to/skills --skill skill-a --skill skill-b
```

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

Without `--skill`, Kitter adds every discovered Skill. Repeat `--skill` to select only specific Skills. Use `--group <group>` to organize them while adding.

### Adopt existing installations

If Skills already exist in user or project Agent directories, scan them first:

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

## Install Skills

### Install into a project

Use the shared Agent directory when compatible Agents in the same project should use a Skill:

```bash
kitter install skill-a skill-b \
  --project /path/to/project \
  --target universal
```

Install only for one Agent when needed:

```bash
kitter install skill-a \
  --project /path/to/project \
  --target codex
```

### Install at user scope

Install genuinely universal Skills at user scope:

```bash
kitter install kitter \
  --project "$HOME" \
  --target universal
```

Install globally for only one Agent:

```bash
kitter install <skill> \
  --project "$HOME" \
  --target codex
```

Available targets include `universal`, `codex`, `claude`, `cursor`, `opencode`, `pi`, `grok`, `antigravity`, `droid`, and `copilot`. Repeat `--target` to install into several targets in one operation.

## Common workflows

### Add one Skill from GitHub and use it in a project

```bash
kitter add npx https://github.com/owner/repository --skill skill-a
kitter install skill-a --project /path/to/project --target universal
kitter project /path/to/project
```

### Share one Skill across several projects

```bash
kitter install skill-a --project /path/to/project-a --target universal
kitter install skill-a --project /path/to/project-b --target universal
```

Both projects now link to the same managed source, so they do not need separate updates.

### Move a project-specific Skill out of global scope

```bash
kitter uninstall skill-a --project "$HOME"
kitter install skill-a --project /path/to/project --target universal
```

### Adopt a Skill from one project and reuse it elsewhere

```bash
kitter adopt /path/to/project-a --json
kitter adopt /path/to/project-a --source /exact/source/path
kitter install skill-a --project /path/to/project-b --target universal
```

## Inspect effective Skills

Inspect user-level effective Skills:

```bash
kitter project "$HOME"
```

Inspect direct installations, Agent-visible Skills, and context estimates for a project:

```bash
kitter project /path/to/project
kitter project /path/to/project --agent codex
```

Inspect filesystem Skills or plugin-provided Skills separately:

```bash
kitter project /path/to/project --view skills
kitter project /path/to/project --view plugins
kitter project /path/to/project --agent codex --view plugins --json
```

Use these results to confirm that the project has the Skills it needs and to detect capabilities already supplied globally or by plugins.

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

Deleting a group keeps its Skills by default. Use `--delete-skills` only when the Skills themselves should also be removed.

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

## Update Skills

Check managed Skills for updates:

```bash
kitter check
kitter check --json
```

Update selected Skills or every managed source:

```bash
kitter update skill-a skill-b
kitter update --all
```

Projects installed through managed links continue using the updated source. Update adopted external sources with their original tool; Kitter does not overwrite them.

## Uninstall and remove Skills

Uninstall a Skill from one project:

```bash
kitter uninstall skill-a --project /path/to/project
```

Uninstall it only from one Agent target:

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

Stop managing a Skill in the Kitter library:

```bash
kitter remove skill-a
```

`uninstall` changes where a Skill is installed. `remove` removes it from the Kitter library and cleans up installations that Kitter still manages.

External symlinks and real directories are preserved by default. Use `--include-unmanaged` for an exact path only after confirming that it should also be removed.

## Change the library location

```bash
kitter library
kitter library --set /absolute/path/to/skills
```

`--set` selects a new library location but does not move the old library automatically. Move existing Skills first, or be prepared to add and adopt them again in the new location.

For complete command options, use the relevant help command:

```bash
kitter --help
kitter install --help
kitter adopt --help
```
