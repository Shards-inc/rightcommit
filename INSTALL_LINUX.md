# Installing rightcommit on Linux

This guide installs rightcommit directly on Linux, without Docker. The app is compiled on
your computer. For the quickest start with Docker, see [INSTALL.md](INSTALL.md); with Podman, see [INSTALL_PODMAN.md](INSTALL_PODMAN.md).

It also works inside **WSL2** on Windows (for example Ubuntu from the Microsoft Store).

> With a native installation every account in the app runs as the same system user.
> Full isolation between users (a separate system user for each account) is only
> available with Docker or Podman.

## Step 1 — Build tools

On Ubuntu or Debian:

```sh
sudo apt update
sudo apt install -y build-essential protobuf-compiler libprotobuf-dev cmake pkg-config libssl-dev git curl python3 ripgrep
```

On Fedora:

```sh
sudo dnf install -y gcc gcc-c++ make protobuf-compiler protobuf-devel cmake pkgconf-pkg-config openssl-devel git curl python3 ripgrep
```

## Step 2 — Node.js 22

Needed to download the agents. Install it with the official `nvm` tool:

```sh
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
. "$HOME/.nvm/nvm.sh"
nvm install 22
```

## Step 3 — Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustup toolchain install 1.95.0
```

## Step 4 — rightcommit

From crates.io (recommended):

```sh
cargo +1.95.0 install rightcommit --locked
mkdir -p ~/rightcommit
cd ~/rightcommit
rightcommit setup
rightcommit
```

`cargo install` compiles the app, which takes several minutes. `rightcommit setup` then
downloads the agents (Codex, Claude Code, Pi) into the `data` folder, without `sudo`.

**Keep the terminal open** while you use the app. Now go to **[Open the app](#open-the-app)**.

Or from source, if you want to change the code:

```sh
cd ~
git clone https://github.com/rightcommit/rightcommit.git
cd rightcommit
cargo +1.95.0 run --locked -- setup
cargo +1.95.0 run --locked
```

From source, use `cargo +1.95.0 run --locked` wherever this guide says `rightcommit`.

## Everyday use

- **Stop the app:** press **Ctrl + C** in its terminal.
- **Start it again:** in a new terminal run `. "$HOME/.cargo/env"` and `. "$HOME/.nvm/nvm.sh"`,
  go to the same folder (`cd ~/rightcommit`) and run `rightcommit`.
  Always start from the same folder: that is where your data lives.
- **Update:** repeat `cargo +1.95.0 install rightcommit --locked`, then `rightcommit setup`
  in `~/rightcommit`. From source: `git pull`, then `cargo +1.95.0 run --locked -- setup`.

## Your own project folders

To work on folders from your computer, use a **Source** node of type **Folder in /sources**.
Put your project folders in `~/rightcommit/sources` and start rightcommit this way:

```sh
mkdir -p ~/rightcommit/sources
cd ~/rightcommit
NODO_SOURCES="$PWD/sources" rightcommit
```

## Open the app

1. Open your browser at **[http://localhost:8080](http://localhost:8080)**.
2. Create the first account with an email and a password: it becomes the **administrator**.
3. Open **Agents and connections**, paste the API key of the agent you want to use
   (Codex, Claude Code or Pi) and save.

Done: you can create your first workflow.

## Troubleshooting

| Problem | What to do |
|---|---|
| `command not found: rightcommit` or `cargo` | Run `. "$HOME/.cargo/env"` in the terminal, or open a new one. |
| `npm: not found` during setup | Run `. "$HOME/.nvm/nvm.sh"`, or repeat step 2. |
| The `localhost:8080` page doesn't open | Check that the terminal running rightcommit is still open. |
| Error about port 8080 already in use | Another app, or another copy of rightcommit (for example in Docker), is using the port. Stop it. |
| You can't find data you created earlier | Start rightcommit from the same folder as before. Docker and this installation keep separate data. |

If you change the examples or extensions in the source, regenerate the bundled resources with
`python3 scripts/bundle-assets.py` (new files must be tracked by Git). Optional connections may
need extra tools (for example `twg` for Atlassian Teamwork Graph).
