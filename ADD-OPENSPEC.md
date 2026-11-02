# Using OpenSpec in rightcommit

[OpenSpec](https://github.com/Fission-AI/openspec) is a spec-driven workflow for AI agents:
propose a change, implement it, then archive it. This guide uses it as an example of how to
add a framework that brings its own skills to Claude Code, Codex and Pi.

It takes three steps: add OpenSpec to your project, make its command available to the agents,
and use it in a workflow.

## Step 1 — Add OpenSpec to your project

On your computer (you need Node.js 22.12 or newer):

```sh
npm install -g @fission-ai/openspec@latest
```

Then, in your project folder:

```sh
openspec init --tools claude,codex,pi
git add .
git commit -m "Add OpenSpec"
```

This adds the OpenSpec skills for the three agents (`.claude/`, `.agents/skills/`, `.pi/`)
and an `openspec/` folder for your specs.

## Step 2 — Make the `openspec` command available to the agents

The agents run the `openspec` command while they work, so it must be installed where
rightcommit runs.

**With Docker or Podman:** open the `Dockerfile` in the rightcommit folder and, right after the
`npm install --global …` line that installs the agents, add this line:

```dockerfile
RUN npm install -g @fission-ai/openspec@1.16.0 && npm cache clean --force
```

Then rebuild:

```sh
docker compose up -d --build
```

(With Podman: `podman compose up -d --build`.)

**With a local installation** (without containers): nothing to do. The command you
installed in step 1 is already available to the agents.

## Step 3 — Use it in a workflow

1. Make your project available to rightcommit: with Docker or Podman, put it in the `sources`
   folder of rightcommit; or push it to a Git repository.
2. In a workflow, add a **Source** node that points to it (**Folder in /sources** or
   **HTTPS Git repository**).
3. Add an **Agent** node with the runtime you want: Claude Code, Codex or Pi.
4. Add **Task** nodes that use that agent, with prompts like these:

   | Task | Prompt |
   |---|---|
   | Propose | `Use the openspec-propose skill to propose this change: <your idea>` |
   | Implement | `Use the openspec-apply-change skill to implement the change.` |
   | Archive | `Use the openspec-archive-change skill to archive the change.` |

   Leave the **Skill name** field empty: it is only for Spec Kit skills.
5. Optionally, add a **Review** node after "Propose" to approve the plan before any code is written.

The skill names are the same for Claude Code, Codex and Pi, so the same workflow works with
any of the three.

## The same recipe for other frameworks

Most agent frameworks follow the same pattern:

1. **Their files go in your project** (skills, commands or prompts), committed to Git.
2. **Their command-line tool**, if they have one, must be installed where rightcommit runs:
   in the `Dockerfile` for Docker and Podman, or on your computer for a local installation.
3. **Your Task prompts name their skills.**

To add tools or skills to Pi for one account instead, see [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md).
