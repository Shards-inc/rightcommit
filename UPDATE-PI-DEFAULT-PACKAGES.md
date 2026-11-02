# Updating Pi's default packages

rightcommit gives every Pi agent three tools by default. They all live in `pi-extensions/`:

| Tool | Comes from | How to update |
|---|---|---|
| `web_search` | npm package `pi-web-search` | new npm version |
| `todo` | npm package `@juicesharp/rpiv-todo` | new npm version |
| `web_fetch` | rightcommit's own file `pi-extensions/web-fetch.ts` | edit the file |

## 1. See what is new

```sh
cd pi-extensions
npm ci
npm outdated
```

`npm outdated` lists only the packages that have a newer version.

Read the changelog (or source) of each package you want to update. Check that its tools
**keep the same names** (`web_search`, `todo`): rightcommit turns them on by name.

## 2. Update

**npm packages**, with exact versions:

```sh
npm install --save-exact pi-web-search@NEW_VERSION @juicesharp/rpiv-todo@NEW_VERSION
```

The libraries used by `web_fetch` (`@mozilla/readability`, `linkedom`, `turndown`) are
updated the same way.

**`web_fetch`:** edit `pi-extensions/web-fetch.ts`.

Then, from the repository root, refresh the bundled resources:

```sh
cd ..
python3 scripts/bundle-assets.py
```

## 3. Check

```sh
cargo test --locked
docker compose up -d --build
```

Then run a short workflow with Pi and check in the Activity Log that `web_fetch`,
`web_search` and `todo` still work.

## 4. Ship

Commit `pi-extensions/package.json`, `pi-extensions/package-lock.json`,
`distribution/assets.json` and, if you changed it, `pi-extensions/web-fetch.ts`.

- **Docker and Podman** users get the update when they rebuild (`up -d --build`).
- **Local installations** get it with the next release (`brew upgrade rightcommit` or
  `cargo install`, from source `git pull`), followed by `rightcommit setup`.

## When you update Pi itself

The Pi version is set in the `Dockerfile` (`PI_VERSION`). The default packages use Pi's
extension API, so after changing it, repeat step 3 to make sure the three tools still load.
