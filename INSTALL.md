# Installing rightcommit with Docker

rightcommit supports one or more users with separate settings and agent homes. Pi, Codex
and Claude Code are available to every user. Before connecting an agent, read
[Provider terms and permitted use](PROVIDER_TERMS.md).

The fastest way to run rightcommit, on **macOS, Windows or Linux**. Docker contains
everything the app needs: you don't install anything else.

**You need:** a 64-bit computer, an Internet connection and about 20–30 minutes.
The first start downloads a lot of files, so some steps can take a few minutes.

Other ways to install:

- with Podman instead of Docker: [INSTALL_PODMAN.md](INSTALL_PODMAN.md)
- without containers: [INSTALL_MAC.md](INSTALL_MAC.md) or [INSTALL_LINUX.md](INSTALL_LINUX.md)

## Step 1 — Install Docker

### macOS

1. Download [Docker Desktop](https://www.docker.com/products/docker-desktop/).
   If it asks which version, check your Mac in the **Apple** menu () → **About This Mac**:
   next to **Chip** you see *Apple* (M1, M2…) or *Intel*.
2. Open the downloaded file and drag Docker into the **Applications** folder.
3. Open **Docker** from Applications and wait until it says it is running.

### Windows

You need 64-bit Windows 10 or 11.

1. Download [Docker Desktop for Windows](https://www.docker.com/products/docker-desktop/).
2. Run the installer and keep the option that uses **WSL 2** selected.
3. If the installer asks, **restart your computer**.
4. Open **Docker Desktop** from the Start menu and wait until it says it is running.
   If it asks you to update WSL, accept.

### Linux

Open a terminal and run Docker's official installation script:

```sh
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker $USER
```

**Log out and log back in** (or restart), then check that `docker compose version` shows a
version number.

## Step 2 — Download rightcommit

1. On the [project's GitHub page](https://github.com/rightcommit/rightcommit) click **Code** → **Download ZIP**.
2. Extract the ZIP file (double-click on macOS; right-click → **Extract All** on Windows).

## Step 3 — Start the app

Open a terminal in the extracted folder (the one that contains **compose.yaml**):

- **macOS or Linux:** in the terminal type `cd ` (with a space), drag the folder into the
  window and press **Return**.
- **Windows:** in File Explorer right-click an empty area of the folder → **Open in Terminal**.

Then run:

```sh
docker compose up -d --build
```

The first time this can take 10–30 minutes. When the cursor comes back the app is running,
and you can close the terminal.

## Step 4 — Open the app

1. Open your browser at **[http://localhost:8080](http://localhost:8080)**.
2. Create the first account with an email and a password: it becomes the **administrator**.
3. Open **Agents and connections**, paste the API key of the agent you want to use
   (Codex, Claude Code or Pi) and save.

Done: you can create your first workflow.

## Everyday use

Run these commands from the rightcommit folder, while Docker is running.

- **Stop the app:** `docker compose down`
- **Start it again:** `docker compose up -d`
- **Update it** after downloading a new version: `docker compose up -d --build`
- **See errors:** `docker compose logs --tail=100`

> ⚠️ Never use `docker compose down -v`: it **deletes all the app's data**
> (the `rightcommit-data` volume).

## Your own project folders

To work on folders from your computer, put them in the `sources` folder inside the
rightcommit folder, then use a **Source** node of type **Folder in /sources**.

## Troubleshooting

| Problem | What to do |
|---|---|
| The `localhost:8080` page doesn't open | Check that Docker is running, then look at the errors with `docker compose logs --tail=100`. |
| Error about port 8080 already in use | Another app is using the port. Stop it, or stop other copies of rightcommit. |
| Windows: Docker Desktop says WSL is missing or needs an update | Open PowerShell as administrator, run `wsl --update`, then restart Docker Desktop. |
| Windows: the build fails with errors about `install-twg.sh` or `\r` | Your copy was downloaded before line endings were fixed. Download the project again. |
| Linux: `permission denied` from Docker | Log out and back in after `usermod`, or use `sudo docker compose …`. |

## Next steps

- Give the Pi agent extra tools: [ADD-PI-PACKAGES.md](ADD-PI-PACKAGES.md)
- Add a skill to all agents: [ADD_A_SKILL.md](ADD_A_SKILL.md)
- Use a framework such as OpenSpec: [ADD-OPENSPEC.md](ADD-OPENSPEC.md)
