# Adding a skill

A skill is a folder with a `SKILL.md` file that teaches agents how to do a task.
`add_skill.sh` adds it to your project for **Claude Code, Codex and Pi** at once.

Your **project** is the folder or Git repository a workflow works on, set in its
**Source** node. The skill is saved inside it, so it works the same with a local
installation, Docker or Podman.

## Where is my project?

| Source type | Project folder to pass to the script |
|---|---|
| **Folder in /sources**, with Docker or Podman | `sources/my-app` inside the rightcommit folder |
| **Folder in /sources**, with a local installation | `~/rightcommit/sources/my-app` (see [INSTALL_MAC.md](INSTALL_MAC.md#your-own-project-folders) or [INSTALL_LINUX.md](INSTALL_LINUX.md#your-own-project-folders)) |
| **HTTPS Git repository** | your local copy of the repository; then commit and push |

## 1. Check the skill

`SKILL.md` must start like this:

```markdown
---
name: my-skill
description: What the skill does and when to use it.
---
```

The name uses lowercase letters, numbers and hyphens.

## 2. Add it to your project

From the rightcommit folder:

```sh
./add_skill.sh path/to/my-skill path/to/your-project
```

You can pass a skill folder or a single `SKILL.md` file.
Running it again updates the skill.

## 3. Commit it

In your project:

```sh
git add .claude/skills .agents/skills
git commit -m "Add my-skill skill"
```

## 4. Use it

In a rightcommit **Task** prompt, write:

```text
Use the my-skill skill to ...
```

That's all. Every user who runs a workflow on this project can now use the skill,
with any of the three agents.

> **On Windows**, use the PowerShell version, from the rightcommit folder:
>
> ```powershell
> powershell -ExecutionPolicy Bypass -File .\add_skill.ps1 path\to\my-skill path\to\your-project
> ```

## Example

Create a small skill that writes release notes:

```sh
mkdir -p release-notes
cat > release-notes/SKILL.md <<'EOF'
---
name: release-notes
description: Write release notes from the git history. Use when asked to prepare release notes.
---
1. Run `git log --oneline` since the last tag.
2. Group the commits into Features, Fixes and Other.
3. Write the result to RELEASE_NOTES.md.
EOF
```

Add it to your project and commit:

```sh
./add_skill.sh release-notes ~/projects/my-app
cd ~/projects/my-app
git add .claude/skills .agents/skills
git commit -m "Add release-notes skill"
```

Then, in a Task prompt:

```text
Use the release-notes skill to prepare the notes for version 1.2.
```
