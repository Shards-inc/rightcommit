# Configuring GitHub and running the SDLC examples

The local app includes four GitHub examples under **New workflow → SDLC with
Connections**. Native Connection nodes use the saved personal credentials; Pi uses
their returned evidence and an isolated repository copy to write reports. All catalog
titles, setup instructions and prompts are available in English and Italian.

| Template | Purpose | Output |
|---|---|---|
| GitHub · Connection and issue access test | Read an existing issue and document observed access. | `output/github-connection.md` |
| GitHub · Issue to requirements and tests | Turn an issue and repository evidence into draft requirements, acceptance criteria and a proposed test plan. | `output/github-requirements.md` |
| GitHub · Commit CI readiness | Verify checks for the exact Source HEAD. | `output/github-ci-readiness.md` |
| GitHub · Set up demo CI | Prepare and, after confirmation, publish a PHP syntax workflow, then verify the published commit. | `output/github-ci-setup.md` |

The two issue examples and CI readiness perform remote reads only. CI setup is a separate
write example: it creates one workflow file on the default branch after confirmation.
None of these templates merges or deploys. Generated requirements still need review.

## 1. Create a GitHub token

1. Sign in to GitHub and open **Settings → Developer settings → Personal access tokens
   → Fine-grained tokens → Generate new token**.
2. Complete GitHub's identity verification if requested.
3. Give the token a recognizable name, such as `rightcommit-demo-sdlc`, and a short
   expiration appropriate for the trial, such as seven days.
4. Select the **Resource owner** that owns the repository.
5. Choose **Only select repositories** and select the repository used for the demo.
6. Add the repository permission **Issues → Read-only**. GitHub also includes the
   required **Metadata → Read-only** permission. No account or write permissions are
   needed for these issue examples.
7. Review the selection, generate the token, and copy it directly into the local app.
   Store it securely if needed; do not put it in a workflow, Source, issue, report or chat.

Fine-grained tokens limit access by owner, repository and permission. Organization
policy may require approval. See GitHub's [personal access token guide](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).

## 2. Save the connection and configure Pi

In **Personal settings → Connections → GitHub**, enter:

| Field | Value |
|---|---|
| Base URL HTTPS | `https://api.github.com` |
| Personal token | The token just generated |

Set **Owner / organization** and **Repository** in each GitHub Connection node. Each node can target a different repository; personal credentials are shared. Existing personal repository defaults remain a fallback for older workflows.

Click **Save settings**. A saved token is hidden when the form reopens. Leaving the
secret field empty preserves it; the removal button explicitly clears it.

In **Personal settings → Agents → Pi**, configure a provider, model and credentials,
then save. GitHub credentials belong to the native connector, not to the agent.

The connection does not configure Git clone authentication. The simplest trial uses a
public HTTPS repository. Private Source cloning requires a separately supported Git
credential setup; never embed a token in a repository URL.

## 3. Prepare a personal workflow

1. Choose either issue template in **New workflow → SDLC with Connections**.
2. Name and save your personal copy.
3. Open **Repository to inspect**. Set **Source type → HTTPS Git repository** and
   **Path or URL** to the matching repository, for example
   `https://github.com/OWNER/REPOSITORY.git`. Save the node.
4. Open **Read GitHub issue**. Keep provider `github` and operation `read_ticket`.
   Set **Ticket ID / local file / base branch** (the node's `location`) to an existing
   issue number, for example `1`. Save the node.
5. Set **Owner / organization** and **Repository** on the GitHub node and confirm they
   match the Source. An explicit node repository overrides the saved personal default.
   The default is only a fallback for older copies with empty node targets. A node can
   use separate owner and repository fields, or a full `OWNER/REPOSITORY` repository value.

Remote Sources are cloned into the run workspace. The issue number is not an issue URL
or a repository name. Select an actual issue; the GitHub issues endpoint can also
return pull requests, which are outside these two demonstrations.

## 4. Run the connection example

1. Select the desired language in the app's **Language / Lingua** selector.
2. Open **GitHub · Connection and issue access test** and choose **Run workflow → Run now**.
3. Keep **Interactive** mode. Add optional context, then click **Start**.
4. Verify that **Read GitHub issue** completes before **Evidence report**.
5. Open **Run files** and read `output/github-connection.md`.

The report records the returned issue number, title, URL and observed repository,
identifies missing evidence, and states the limits of the check. Successful access to a
public issue does not establish token identity, private-repository access or write
permissions. It is a connectivity and issue-read demonstration.

## 5. Run the requirements example

Repeat the setup for **GitHub · Issue to requirements and tests**, using the same Source
and issue number. Start a new run and open `output/github-requirements.md`.

Review the document for:

- Issue URL and number, with traceability to repository files.
- Explicit requests kept separate from proposed requirements.
- Testable acceptance criteria and a proposed test plan.
- Missing information and open questions rather than invented requirements.

Choose a real issue from your selected repository. If it is historical, treat its
content as evidence rather than confirmation of a current product priority. The
requirements example proposes tests; it does not execute them or claim delivery.

## 6. Report language

The GitHub templates use `{{app_report_language}}` in their report instructions. The
app sends its selected language when starting a run; the server resolves the placeholder
to Italian or English in that run's workflow snapshot. The saved workflow keeps the
placeholder, so the same personal copy can produce reports in either language.

Select the language **before clicking Start**. Changing the UI language during or after
a run does not translate an existing report. Technical identifiers, paths and quoted
source evidence may remain in their original language. API clients can provide
`"locale": "it"` or `"locale": "en"` when creating a run; omission defaults to English.
Schedules capture the UI language when saved and reuse it for future occurrences.
Older personal copies with a hard-coded language must adopt the updated report prompt
or be recreated from the updated catalog. Existing run snapshots and reports remain
unchanged.

## 7. Optional: commit CI readiness

A third template, **GitHub · Commit CI readiness**, reads
`GET /repos/{owner}/{repo}/commits/{HEAD}/check-runs?per_page=100` for the Source checkout's
exact HEAD. It writes `output/github-ci-readiness.md` only if the native check passes.

This is separate from the two issue examples. GitHub currently documents Checks API
access as a limitation of fine-grained personal access tokens; do not look for a
**Checks** permission in that token form. Public check runs may be readable without
authentication. For other access requirements, consult the supported token types in the
[Checks API documentation](https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference)
and the [token limitations](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#fine-grained-personal-access-tokens-limitations).
Do not expand the issue-demo token's access just to make this optional check pass.

The native validator requires at least one check, a complete response, and every returned
check to be completed with conclusion `success`. No checks, pending/failed/skipped checks
or more results than the single page contains stop the run before report generation.
It does not query commit statuses or certify all Actions workflows or branch-protection
requirements. A CI pass does not authorize merge or deployment.

An endpoint response with `total_count: 0` means that no check runs are available for
that reference. The workflow stops in this case; that response alone does not indicate
a token-authentication failure.

## Troubleshooting

| Symptom | What to check |
|---|---|
| Missing connector token | Save the token in the same app account used to run the workflow. |
| HTTP 401 | Token expiration, revocation or an incorrectly copied value. |
| HTTP 403 | Repository permissions, organization approval/policy, rate limits, or unsupported token/API combination. |
| HTTP 404 | Repository spelling, issue number and access. Private resources can also be hidden as 404. |
| Clone fails before the GitHub node | Source URL, network and separate Git clone authentication. |
| GitHub completes but report fails | Pi provider/model credentials and the agent's reported error. |
| Report uses an old fixed language | Update the personal copy's report prompt and start a new run after selecting the app language. |
| CI says incomplete or not positive | Check whether that exact commit has any check runs and whether all returned checks satisfy the validator. |

Keep the run's native evidence, generated report and error message together when
investigating a failure. Never include tokens in diagnostic material.


## Full GitHub API requests

Choose **GitHub → GitHub API request** in a Connection node. The searchable endpoint
catalog bundles all 1,254 REST operations in the official OpenAPI snapshot, including
issues, pull requests and merges, Actions, repositories, branches, releases, teams,
organizations, security, webhooks and settings. Type an operation ID such as
`issues/create` or `actions/create-workflow-dispatch`. Selecting an endpoint fills its
method and path; complete the remaining path parameters, query parameters and body
using its **GitHub endpoint documentation** link. Required body fields differ by endpoint.

`{owner}` and `{repo}` come from the node's repository, and `{org}` from its organization.
Other path values go into **Path parameters (JSON)**. Query parameters accept string,
number, boolean and array values. Additional headers allow content negotiation,
conditional requests and API version overrides; authentication and transport headers
are managed by the connection. The default REST version is `2026-03-10`.

Example: select `issues/create`, set owner and repository, leave query parameters empty,
and enter this JSON request body:

```json
{"title":"Review deployment checklist","body":"Check release readiness.","labels":["documentation"]}
```

A custom method and path also work without selecting a catalog entry. For GraphQL,
use **POST**, path **/graphql**, and a JSON body such as:

```json
{"query":"query { viewer { login } }","variables":{}}
```

All POST, PUT, PATCH and DELETE requests pause for the workflow's external-effect
approval, including GraphQL queries (POST). The preview includes the resolved target,
method, headers and exact body. Changing request fields invalidates the previous
approval. Credentials stay in personal settings. Requests still need the GitHub token
type and permissions required by the endpoint; the catalog does not grant permissions
or create GitHub App credentials automatically.

Release asset uploads use the official `https://uploads.github.com` host. Select
`repos/upload-release-asset`, fill `release_id`, set a `name` query parameter, and paste
the asset as Base64. Set an appropriate `Content-Type` header if it differs from
`application/octet-stream`. JSON, plain text and Base64 bodies are supported. The
encoded request field and response body are limited to 8 MiB; large file transfers
need a separate tool.

Results contain HTTP status, selected response headers and the response body, including
empty 204 responses. Binary responses use Base64. GraphQL `errors` fail the node even
when HTTP status is 200. Pagination is explicit: one request fetches one page; use
`page`, `per_page` or cursor parameters and inspect the returned `Link` header (or
GraphQL `pageInfo`). GitHub rate limits and `Retry-After` are reported. Writes are never
retried automatically: reconcile the remote result before retrying an uncertain write.
GET download redirects to GitHub content hosts are followed without forwarding the
token to a different origin. Other redirect destinations fail explicitly.

The catalog is bundled locally and needs no runtime schema download. Refresh it with
`python3 scripts/sync-github-api.py`; provenance and schema SHA-256 appear in
`web/github-api.js`, and its MIT license is included under `licenses/`.

References: [REST requests](https://docs.github.com/en/rest/using-the-rest-api/getting-started-with-the-rest-api),
[REST pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api),
[GraphQL calls](https://docs.github.com/en/graphql/guides/forming-calls-with-graphql),
[official REST schema](https://github.com/github/rest-api-description).

## 10. Set up demo CI from the catalog

**GitHub · Set up demo CI** (Italian: **GitHub · Configura una CI dimostrativa**) is a
separate, writable demonstration for a PHP repository. Its report follows the app
language selected at run start. It installs the exact bundled
[`demo-ci.yml`](../examples/github-ci/demo-ci.yml), with a PHP syntax job and a summary
that explicitly excludes device and functional testing.

1. Choose a demonstration repository with GitHub Actions enabled. The default branch
   must permit direct commits, and `.github/workflows/demo-ci.yml` must not already exist.
2. Save a token authorized for this repository in Personal settings. Creating files under
   `.github/workflows` requires **Contents: write** and **Workflows: write** for fine-grained
   tokens (classic tokens require the applicable repository access and `workflow` scope).
   Check-run verification also requires supported Checks API access as described above.
   See the official [repository contents API](https://docs.github.com/en/rest/repos/contents#create-or-update-file-contents).
3. Create a personal copy of the template. Select an isolated copy of the matching
   repository in **Source**. Set the same **Owner / organization** and **Repository** on
   **Read target repository**, **Publish demo CI**, and **Verify published commit checks**.
4. Start in **Interactive** mode. The preparation task checks the target and writes
   `output/github-ci-plan.md` with the full proposed YAML and its hash.
5. At the native write confirmation, inspect the repository, default branch, path and
   fixed request body. Approval creates one file directly on the default branch and
   starts Actions through the push event. It does not open a PR. Reject to stop.
6. The next task records the native publication response in
   `output/github-ci-publication.json` and fetches/checks out only the returned commit SHA
   in the isolated Source. No agent reads the saved token or uses it in shell commands.
7. At **Wait for GitHub Actions**, open Actions for that exact commit. Approve only when
   all its checks have completed successfully; reject on failure. This wait is manual,
   with no automatic polling. The next native node verifies the check runs again.
8. Read `output/github-ci-setup.md`: publication evidence, SHA agreement, actual check
   outcomes, links and limitations. Success does not authorize merge or deployment.

This is a fixed PHP example, not an automatic CI generator for every language. Editing
only the prepared local YAML does not change the native request's base64 payload. To adapt
it, update that payload and the preparation instructions together, then review both.
The API call intentionally omits the existing file SHA: it cannot silently overwrite a
workflow. A pre-existing file, branch rule, permission problem or missing PHP files stops
the run. It never bypasses repository rules or retries uncertain writes automatically.
For an existing CI, use **GitHub · Commit CI readiness**, including after a successful setup;
re-running setup is not an update operation.

## Recheck existing personal workflows

Catalog changes do not automatically update saved personal copies or historical runs.
Before a regression run, compare each copy with its corresponding current template:

- Preserve the actual Source, issue number and intended repository bindings. Set explicit
  owner/repository targets on every GitHub node instead of relying on a mutable default.
- Adopt corrected prompts and setup notes while retaining `{{app_report_language}}`.
- Start a fresh run in the selected language. For CI readiness, wait for Actions on the
  published Source HEAD first and confirm the report SHA matches the check-run SHA.
- Check both the terminal run status and the actual report in **Run files**. An old
  successful report does not validate a new run, and a public read does not prove write access.

Rerun CI readiness for an existing CI. CI setup is create-only and must not be rerun to
overwrite an existing workflow. Never change branch rules, replace a workflow or expand
token access merely to obtain a passing regression result.
