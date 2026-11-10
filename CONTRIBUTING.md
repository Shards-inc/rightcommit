# Contributing to RightCommit

Thank you for your interest in contributing. Pull requests are welcome.

## Contributor License Agreement (CLA)

Before your first pull request can be merged, you must sign the [Contributor License Agreement](CLA.md).

1. Open a pull request.
2. The CLA bot will comment on it with a link to the agreement if you have not signed yet.
3. Read the CLA, then reply to the bot's comment with exactly this sentence:

   `I have read the CLA Document and I hereby sign the CLA`

4. The bot records your signature and updates the check on the pull request. If the check does not update, comment `recheck`.

Signing is a one-time step and applies to all your future contributions. Pull requests from contributors who have not signed cannot be merged. If you contribute on behalf of your employer, make sure you are authorized to do so.

## Workflow

1. Fork the repository and create a topic branch from `main`.
2. Keep changes focused: one logical change per pull request.
3. Make sure the build and the automated checks pass locally before opening the pull request.
4. Describe what the change does and why in the pull request description.

## Development

Use Rust 1.95, as declared in `Cargo.toml`. The install guides cover rustup setup and platform prerequisites: [Linux](INSTALL_LINUX.md) and [macOS](INSTALL_MAC.md).

The default branch is `main`. Before opening a pull request, run:

```sh
cargo test --locked
```

This is the same test command used by CI. Optional Playwright/browser smoke checks live in `scripts/` for maintainers; they are not required for documentation or small unit-test changes and are not run in CI today.

## Commit messages

Use short, imperative subject lines (for example `Add retry limit to review step`) and explain the motivation in the body when it is not obvious.

## Reporting bugs and proposing features

Open an issue with clear steps to reproduce (for bugs) or the problem you want to solve (for features). For larger changes, please open an issue first so the approach can be discussed before you invest time.

## License

By contributing you agree that your contributions are licensed under the terms in [LICENSE](LICENSE) and the CLA.
