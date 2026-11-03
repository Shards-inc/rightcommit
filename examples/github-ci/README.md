# Demo GitHub CI

`demo-ci.yml` is the exact fixed payload used by the bilingual **GitHub · Set up demo CI**
catalog template. It lints tracked PHP files on Ubuntu and records its limited coverage
in the Actions job summary. It does not contact ONVIF devices or use secrets.

The template prepares an isolated repository copy, requests confirmation before creating
`.github/workflows/demo-ci.yml` on the repository's default branch, then checks out the
exact commit returned by GitHub. A human review waits for Actions before the native
check-run verification. A pre-existing file or a protected default branch blocks creation;
use **GitHub · Commit CI readiness** to inspect an existing CI instead.

See [Configuring GitHub](../../guides/CONFIGURING_GITHUB.md) for permissions and setup.
