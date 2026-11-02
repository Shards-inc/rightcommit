# Installing rightcommit on macOS

This guide installs rightcommit directly on your Mac, without Docker. The app is compiled on
your Mac. For the quickest start with Docker, see [INSTALL.md](INSTALL.md); with Podman, see [INSTALL_PODMAN.md](INSTALL_PODMAN.md).

**You need:** a Mac, an Internet connection and about 30 minutes.

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

## Step 1 — Install Homebrew (only if you don't have it)

Homebrew is a program that installs other apps from Terminal. Check whether you already have it:

```sh
brew --version
```

- If you see a version number, go to **step 2**.
- If you see `command not found`, open [brew.sh](https://brew.sh), copy the command
  on that page, paste it into Terminal and follow the instructions. At the end Homebrew shows
  a few commands under **Next steps**: run them, then close and reopen Terminal.

## Step 2 — Build tools

```sh
xcode-select --install
```

Wait for it to finish (if it is already installed, continue), then:

```sh
brew install protobuf node@22 pkg-config openssl cmake ripgrep
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup toolchain install 1.95.0
```

## Step 3 — Get rightcommit

```sh
cd ~
git clone https://github.com/rightcommit/rightcommit.git
cd rightcommit
```

The app and your data live in this `~/rightcommit` folder.

## Step 4 — Set up the agents (once)

In **every new Terminal**, before starting rightcommit, run:

```sh
source "$HOME/.cargo/env"
export PATH="$(brew --prefix node@22)/bin:$PATH"
```

Then:

```sh
cargo +1.95.0 run --locked -- setup
```

The first time this compiles the app, which takes several minutes. Then it downloads the AI
agents (Codex, Claude Code, Pi). When it finishes you see **Configurazione completata**
("setup complete").

## Step 5 — Start the app

```sh
cargo +1.95.0 run --locked
```

**Keep this Terminal window open** while you use the app.
Now go to **[Open the app](#open-the-app)**.

## Everyday use

- **Stop the app:** in the Terminal window press **Ctrl + C**.
- **Start it again another day:** open Terminal and run

  ```sh
  source "$HOME/.cargo/env"
  export PATH="$(brew --prefix node@22)/bin:$PATH"
  cd ~/rightcommit
  cargo +1.95.0 run --locked
  ```

  Always start from the `~/rightcommit` folder: that is where your data lives.
- **Update to a new version:**

  ```sh
  cd ~/rightcommit
  git pull
  cargo +1.95.0 run --locked -- setup
  cargo +1.95.0 run --locked
  ```

## Your own project folders

To work on folders from your computer, use a **Source** node of type **Folder in /sources**.
Put your project folders in `~/rightcommit/sources` and start rightcommit this way:

```sh
cd ~/rightcommit
NODO_SOURCES="$PWD/sources" cargo +1.95.0 run --locked
```

Git ignores the content of `sources`, so your projects never end up in a commit.

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
| `command not found: cargo` | Run `source "$HOME/.cargo/env"`, or repeat step 2. |
| Errors about `npm` or `node` during setup | Run `export PATH="$(brew --prefix node@22)/bin:$PATH"`, then repeat step 4. |
| The `localhost:8080` page doesn't open | Check that the Terminal window running rightcommit is still open. |
| Error about port 8080 already in use | Another app, or another copy of rightcommit (for example in Docker), is using the port. Stop it. |
| You can't find data you created earlier | Start rightcommit from the same folder as before. Docker and this installation keep separate data. |

## For developers

- If you change the examples or extensions, regenerate the bundled resources first with
  `python3 scripts/bundle-assets.py` (new files must be tracked by Git).
- Optional connections may need extra tools
  (for example `twg` for Atlassian Teamwork Graph).
