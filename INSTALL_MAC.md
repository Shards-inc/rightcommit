# Installing rightcommit on macOS

rightcommit supports one or more users with separate settings and agent homes. Pi, Codex
and Claude Code are available to every user. Before connecting an agent, read
[Provider terms and permitted use](PROVIDER_TERMS.md).

This guide installs rightcommit directly on your Mac, without Docker.
For the quickest start with Docker, see [INSTALL.md](INSTALL.md); with Podman, see [INSTALL_PODMAN.md](INSTALL_PODMAN.md).

**You need:** a Mac, an Internet connection and about 20 minutes.

There are two ways:

- **[Homebrew](#install-with-homebrew)** (recommended): one command installs the app and
  everything it needs.
- **[crates.io or source](#for-developers-cratesio-or-source)**: for developers who already
  use Rust.

> With a native installation every account in the app runs as your Mac user.
> Full isolation between users (a separate system user for each account) is only
> available with Docker or Podman.

## Before you start: using Terminal

Terminal is the Mac app where you type commands.

1. Press **⌘ Cmd + Space**.
2. Type **Terminal** and press **Return**.

**How to run a command:** copy the line from the grey box, paste it into Terminal
with **⌘ Cmd + V** and press **Return**. Wait for the command to finish (the cursor comes back)
before moving on to the next one.

> If Terminal asks for your **Mac password**, type it and press Return.
> Nothing appears on screen while you type: that is normal.

## Install with Homebrew

### Step 1 — Install Homebrew (only if you don't have it)

Homebrew is a program that installs other apps from Terminal. Check whether you already have it:

```sh
brew --version
```

- If you see a version number, go to **step 2**.
- If you see `command not found`, open [brew.sh](https://brew.sh), copy the command
  on that page, paste it into Terminal and follow the instructions. At the end Homebrew shows
  a few commands under **Next steps**: run them, then close and reopen Terminal.

### Step 2 — Install rightcommit

```sh
brew install rightcommit/tap/rightcommit
```

On Apple Silicon Macs with macOS 15 or later, Homebrew downloads rightcommit ready to use in a
few seconds. On Intel Macs and older macOS versions it compiles rightcommit, which takes several
minutes and installs Rust: afterwards `brew autoremove` removes it. Workflows on Rust projects,
such as the bundled helpdesk example, use the Rust in your PATH ([rustup](https://rustup.rs)).

### Step 3 — Create the app folder

```sh
mkdir -p ~/rightcommit
cd ~/rightcommit
```

The app keeps your data in this folder.

### Step 4 — Set up the agents (once)

```sh
rightcommit setup
```

This downloads the AI agents (Codex, Claude Code, Pi). When it finishes you see
**Configurazione completata** ("setup complete").

### Step 5 — Start the app

```sh
rightcommit
```

**Keep this Terminal window open** while you use the app.
Now go to **[Open the app](#open-the-app)**.

### Optional configuration with .env

Create `.env` inside the folder from which you start the app. For the Homebrew
steps above, this is `~/rightcommit/.env`, alongside `data` and `sources`, not
inside Homebrew's installation directory. The Homebrew launcher preserves your
current working directory.

For example:

```dotenv
# 1 allows new accounts to register; 0 keeps public registration closed.
NODO_ALLOW_REGISTRATION=0
# Native listen address and port (RIGHTCOMMIT_PORT is only for Docker Compose).
NODO_LISTEN=127.0.0.1:8080
```

The repository's `.env.example` lists the available settings, grouped by purpose.
Protect your file with `chmod 600 ~/rightcommit/.env`. Stop the app and start it
again from `~/rightcommit` after changes. Settings already exported in Terminal
take precedence over `.env`; empty optional entries do not override native defaults.
The app reads only the `.env` in its working directory and does not search parent
folders. If you configure your own background service, set that service's working
directory to `~/rightcommit` too.

### Everyday use

- **Stop the app:** in the Terminal window press **Ctrl + C**.
- **Start it again another day:** open Terminal and run

  ```sh
  cd ~/rightcommit
  rightcommit
  ```

  Always start from the `~/rightcommit` folder: that is where your data lives.
- **Update to a new version:**

  ```sh
  brew update
  brew upgrade rightcommit
  cd ~/rightcommit
  rightcommit setup
  ```

- **Your own project folders:** to work on folders from your computer, use a **Source** node
  of type **Folder in /sources**. rightcommit creates `~/rightcommit/sources` at its first
  start: put your projects there.

## Open the app

1. Open your browser at **[http://localhost:8080](http://localhost:8080)**.
2. Create the first account with an email and a password: it becomes the **administrator**.
3. Open **Agents and connections**, paste the API key of the agent you want to use
   (Codex, Claude Code or Pi) and save.

Done: you can create your first workflow.

## Troubleshooting

| Problem | What to do |
|---|---|
| `command not found: brew` | Run the **Next steps** commands shown at the end of the Homebrew installation, then close and reopen Terminal. |
| `command not found: rightcommit` | Close and reopen Terminal. If that doesn't help, repeat step 2. |
| `command not found: cargo` (crates.io or source) | Run `source "$HOME/.cargo/env"`. |
| Errors about `npm` or `node` during setup (crates.io or source) | Run `export PATH="$(brew --prefix node@24)/bin:$PATH"`, then repeat the setup. |
| The `localhost:8080` page doesn't open | Check that the Terminal window running rightcommit is still open. |
| Error about port 8080 already in use | Another app, or another copy of rightcommit (for example in Docker), is using the port. Stop it. |
| You can't find data you created earlier | Start rightcommit from the same folder as before. Docker and this installation keep separate data. |

## For developers: crates.io or source

These options compile the app with your own Rust toolchain.

### Shared prerequisites

Once, with [Homebrew](https://brew.sh) installed:

```sh
xcode-select --install
```

Wait for it to finish (if it is already installed, continue), then:

```sh
brew install protobuf node@24 pkg-config openssl cmake ripgrep
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup toolchain install 1.95.0
```

In **every new Terminal**, before using these options:

```sh
source "$HOME/.cargo/env"
export PATH="$(brew --prefix node@24)/bin:$PATH"
```

### From crates.io

```sh
cargo +1.95.0 install rightcommit --locked
mkdir -p ~/rightcommit
cd ~/rightcommit
rightcommit setup
rightcommit
```

- To update, repeat `cargo +1.95.0 install rightcommit --locked`, then `rightcommit setup`.
- Data stays in `~/rightcommit/data`: always start from the same folder.

### From source

```sh
cd ~
git clone https://github.com/rightcommit/rightcommit.git
cd rightcommit
cargo +1.95.0 run --locked -- setup
cargo +1.95.0 run --locked
```

- Next time, `cd ~/rightcommit` and `cargo +1.95.0 run --locked` are enough.
- To update: `git pull`, then `cargo +1.95.0 run --locked -- setup`.
- Your own project folders go in `~/rightcommit/sources` (Git ignores their content).
- If you change the examples or extensions, regenerate the bundled resources first with
  `python3 scripts/bundle-assets.py` (new files must be tracked by Git).
- Optional connections may need extra tools
  (for example `twg` for Atlassian Teamwork Graph).
