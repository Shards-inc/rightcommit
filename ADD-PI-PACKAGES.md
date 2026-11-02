# Adding Pi packages

[Pi packages](https://pi.dev/packages) give the Pi agent new tools and skills.
A package you add works for **one account**, in all its workflows.

> ⚠️ Install only packages you trust: they run with your account's permissions.
> Don't install `pi-web-search`, `@juicesharp/rpiv-todo` or `pi-web-access`:
> rightcommit already includes these tools, and a duplicate stops Pi from working.

## Example: from pi.dev to rightcommit

1. On [pi.dev/packages](https://pi.dev/packages), open the package page and find its
   install command, for example `pi install npm:some-package`.
2. Copy the part after `pi install`: here `npm:some-package`.
3. Run the command for your setup below, with that text in place of `PACKAGE`.

That's all: there is nothing to configure in the app. From the next task, the account's Pi
agents can use the package.

Pick packages that add tools, skills or prompts. Packages made for Pi's own screen (panels,
shortcuts, `/` commands, interactive questions) don't help, because rightcommit runs Pi
without it. If a package needs other programs or API keys, they must be available where
rightcommit runs (inside the container with Docker or Podman).

## Commands

In the commands below, replace `PACKAGE` with the package name (for example
`npm:some-package`). They are for the first account (ID 1).

## With Docker

Open a terminal in the rightcommit folder (the one with `compose.yaml`) and run:

```sh
docker compose exec --user 20001:20001 -e HOME=/data/users/1 -e PI_CODING_AGENT_DIR=/data/users/1/.pi/agent rightcommit pi install PACKAGE
```

## With Podman

Same command, with `podman` instead of `docker`:

```sh
podman compose exec --user 20001:20001 -e HOME=/data/users/1 -e PI_CODING_AGENT_DIR=/data/users/1/.pi/agent rightcommit pi install PACKAGE
```

## Local installation (without containers)

Open a terminal in the folder you start rightcommit from (for example `~/rightcommit`) and run:

```sh
export PATH="$PWD/data/system/tools/bin:$PATH"
HOME="$PWD/data/users/1" PI_CODING_AGENT_DIR="$PWD/data/users/1/.pi/agent" pi install PACKAGE
```

On macOS, first run `export PATH="$(brew --prefix node@24)/bin:$PATH"`.

## Good to know

- **Done:** no restart needed. Pi uses the package from its next task.
- **Remove a package:** same command, with `pi remove PACKAGE` instead of `pi install PACKAGE`.
- **List packages:** same command, with `pi list`.
- **Another account:** change the `1` in every path to the account ID, and `20001` to
  20000 + the ID (account 2 → `20002`).

## For every user (maintainers)

To give a package to all users, add it to rightcommit itself:

```sh
cd pi-extensions
npm install --save-exact some-package@1.2.3
cd ..
python3 scripts/bundle-assets.py
```

Then commit the changed files and rebuild (`docker compose up -d --build`).

To update the packages rightcommit already ships, see
[UPDATE-PI-DEFAULT-PACKAGES.md](UPDATE-PI-DEFAULT-PACKAGES.md).
