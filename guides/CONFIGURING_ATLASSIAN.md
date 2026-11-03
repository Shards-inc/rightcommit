# Configuring Atlassian and running the Jira, Bitbucket and Confluence workflows

This guide explains how to connect Atlassian from the local app's **Personal settings**
and run the three built-in SDLC demonstrations. All account, site, issue and repository
values below are placeholders; replace them with resources you are authorized to access.

| Template under New workflow → SDLC with Connections | Purpose | Main output |
|---|---|---|
| **Jira · Issue to requirements and tests** | Read a Jira issue and prepare draft requirements, acceptance criteria and a proposed test plan. | `output/jira-requirements.md` |
| **Confluence · Page to SDLC briefing** | Read a page and prepare source-traced requirements, decisions, risks and a proposed test checklist. | `output/confluence-sdlc-brief.md` |
| **Jira + Bitbucket · PR review dossier** | Compare a Jira issue with a Bitbucket PR, its build statuses and file-level diff statistics. | `output/atlassian-pr-review.md` |

All three templates are available in English and Italian. They perform remote reads and write
local documentation. They do not implement code, execute tests or pipelines, change
issues, comment on or approve PRs, merge, or deploy.

## 1. Prerequisites

You need:

- A working local app installation with the Atlassian Teamwork Graph CLI (TWG) available.
- A configured **Pi** provider and model in **Personal settings → Agents → Pi**.
- An Atlassian account with access to the Jira site and the issue used for the demonstration.
- For Confluence, an active Confluence product on the selected site and an accessible page.
- For the combined workflow, a Bitbucket workspace, repository and existing pull request
  accessible to that account, plus a scoped Bitbucket API token.

A new Jira site and Bitbucket workspace are not required if you already have suitable
resources. A small documentation-only PR is sufficient for a demonstration. Give the Jira
issue a clear description and acceptance criteria if you want a substantive comparison.
An empty sample issue instead demonstrates how the report handles missing requirements.

## 2. Connect Jira, Confluence and other Atlassian products

1. Open **Personal settings → Connections → Atlassian**.
2. Under **Jira, Confluence and other products**, enter **Atlassian site URL**, for example
   `https://YOUR-SITE.atlassian.net`. Use the site root, without a page path, query or credentials.
3. Click **Save settings** before starting the account connection.
4. Click **Connect Atlassian account**.
5. Follow **Open Atlassian and authorize the connection** when the link appears.
6. In Atlassian, check the selected site and requested permissions. Grant the product
   read permissions needed for the demonstration and account/profile read access.
   The CLI needs account identity access (`read:account`) to complete login.
7. Complete the authorization yourself, then return to the app and wait for the connection result.

Personal Atlassian sign-in uses OAuth. Jira, Confluence and other supported products share
that sign-in, subject to the permissions granted and your existing access. A Jira-only
authorization does not establish Confluence access. Bitbucket has a separate token setup.
See the official [TWG authentication guide](https://atlassian.github.io/twg-cli/getting-started/how-authentication-works/).

The app starts the login flow for its current user; no terminal command is needed for
this interface. OAuth state remains managed by the user's TWG login profile. An existing
working login can be checked before starting another authorization. If login expires or
fails, retry **Connect Atlassian account** and review the permissions, including profile access.

## 3. Create a scoped Bitbucket API token

1. Open your Atlassian account's **Security → API tokens** page:
   [Manage API tokens](https://id.atlassian.com/manage-profile/security/api-tokens).
2. Complete the identity verification if Atlassian requests it.
3. Select **Create API token with scopes**.
4. Give it a recognizable name, such as `sdlc-demo-read`, and a suitable expiration date.
5. Select **Bitbucket** as the app.
6. For these read-only demonstrations, select:

   | Scope | Purpose |
   |---|---|
   | `read:repository:bitbucket` | Read repository information and file-level change evidence. |
   | `read:pullrequest:bitbucket` | Read pull-request information. |
   | `read:user:bitbucket` | Read account identity information used by the integration. |

7. Review the scopes, create the token yourself, and copy it into the local app's token field.

Do not add write, delete or administration scopes for these examples. API token scopes
are separate permissions; one scope does not automatically grant all related access.
Your account must also be allowed to access the target resources. See
[Bitbucket API token permissions](https://support.atlassian.com/bitbucket-cloud/docs/api-token-permissions/).

Keep the token out of workflow JSON, prompts, repository files and chat messages. If it
expires or is revoked, replace it in Personal settings.

## 4. Save Bitbucket access in Personal settings

In **Personal settings → Connections → Atlassian → Bitbucket**, fill in:

| Field | Example / meaning |
|---|---|
| **Atlassian account email** | `user@example.com`, the email associated with the token. |
| **Bitbucket API token** | The scoped token generated above. |
| **Bitbucket repository workspace/repository** | `WORKSPACE/REPOSITORY`, replaced with the actual workspace and repository slugs. |

Click **Save settings**. The Bitbucket token is encrypted in the app's settings store and
is not returned to the browser when settings are reloaded. The connector supplies it to
native TWG calls; it is not placed in agent prompts or workflow command arguments.

A blank token field retains the saved token. Use **Remove saved Bitbucket token** and then
save to remove it. Changing the saved account email without entering a replacement token
clears the previous token rather than silently reusing another account's credentials.

Set the repository for the demonstrations even though the UI permits leaving it empty.
It gives **Test connection** a concrete repository to check. With no repository, the test
attempts a workspace query instead, which may require additional permissions.

## 5. Use Test connection

Click **Test connection** in the Atlassian section. It reports two independent outcomes:

- **Atlassian access:** whether the configured OAuth context can read account identity.
- **Bitbucket:** whether the supplied or saved token can read the specified repository
  (or query workspaces if no repository is specified).

The test uses current form values, retaining the saved token when its field is blank and
the account email matches. Testing does not save edited fields: click **Save settings**
when you want subsequent workflow runs to use those values.

A connected result does not certify every Jira issue, Confluence page, Bitbucket PR,
product permission or write operation. The workflow's native reads verify access to their
specific targets. “Token not configured” is different from a failed credential check.

## 6. Create a personal workflow copy

1. Click **New workflow** and find **SDLC with Connections**.
2. Select one of the three Atlassian templates and name your personal copy.
3. Keep the bundled **Atlassian demo setup** Source. It copies `examples/atlassian`, including
   the evidence-capture helper required by the combined workflow.
4. Keep the configured Pi agent, or choose another supported Pi configuration.
5. Edit the Connection nodes as described below and save them.

TWG command arguments are JSON arrays, not shell scripts. Keep credentials in Personal
settings. The repository field in settings is used by **Test connection**; it does not
replace the explicit repository argument in these workflow nodes.

### Jira issue node — Jira workflows

Open **Read Jira issue** and replace `PROJ-123` with an existing issue key:

```json
["jira", "workitem", "get", "PROJ-123", "--strict", "-o", "json"]
```

Keep the Atlassian/TWG provider (shown as **Bitbucket / Atlassian TWG** in the node editor)
and operation `twg`. The same provider handles Jira, Bitbucket and Confluence.

### Bitbucket detail node — combined workflow

Open **Read Bitbucket PR and statuses**. Replace PR number `1` and `WORKSPACE/REPOSITORY`:

```json
["bitbucket", "pull-requests", "get", "1", "--repo", "WORKSPACE/REPOSITORY", "--statuses", "--strict", "-o", "json"]
```

### Bitbucket statistics node — combined workflow

Open **Read PR file statistics** and set the **same PR number and repository**:

```json
["bitbucket", "pull-requests", "diffstat", "1", "--repo", "WORKSPACE/REPOSITORY", "--limit", "100", "-o", "json"]
```

Use the dedicated `diffstat` command. The combined template does not rely on a detail
request's `--diff` flag to supply file statistics.

## 7. Run Jira → requirements and tests

1. Select **EN** or **IT** in the app before starting.
2. Open your personal **Jira · Issue to requirements and tests** copy.
3. Choose **Run workflow → Run now**, keep **Interactive** mode, and click **Start**.
4. When the workflow pauses for confirmation, inspect the Jira issue key and read command.
5. Approve the intended read and wait for the report task to finish.
6. Open **Run files → `output/jira-requirements.md`**.

Review the issue references, explicit requirements, proposed additions, acceptance
criteria, proposed tests and open questions. Missing requirements should remain explicit;
a generated test plan is not evidence of tests being executed.

## 8. Run Jira + Bitbucket → PR review dossier

Use your configured combined workflow copy and start a new run in the same way. The
execution sequence is:

1. Read the Jira issue, then preserve its native JSON response.
2. Read Bitbucket PR details and build statuses, then preserve that native response.
3. Read file-level diff statistics with the separate `diffstat` command, then preserve them.
4. Verify the three snapshots and generate the dossier from those explicit files.

The app requests confirmation for **each of the three native TWG commands**, including
these reads. Check the issue, repository and PR number before approving. Approving a read
within the workflow does not approve the remote pull request.

The evidence-capture tasks use the immediately preceding node's exact `output_files.stdout`
reference. They copy the original JSON without rewriting it and record its native origin,
byte count and SHA-256. They do not search temporary directories or infer file paths from
timestamps. Missing or invalid evidence prevents successful report generation.

The report runs the bundled evidence verification before using:

| Snapshot | Meaning |
|---|---|
| `output/atlassian-evidence/jira.json` | Native Jira issue response. |
| `output/atlassian-evidence/bitbucket.json` | Native PR details and returned build statuses. |
| `output/atlassian-evidence/diffstat.json` | Native file-level diff statistics. |

Each snapshot has a corresponding `.provenance.json` manifest. The main dossier is
**`output/atlassian-pr-review.md`**. It should cite the snapshots and list changed paths,
file statuses, added/removed lines, totals over returned records, risks and review questions.

## 9. Run Confluence → SDLC briefing

Confluence must be active on the selected Atlassian site, and your account and OAuth
connection must have permission to read the target page. Jira access alone does not
establish this. Open the site's Confluence app first and choose an existing page whose
content you are authorized to use. Copy its page URL or numeric page ID.

Create a **Confluence · Page to SDLC briefing** workflow copy. Open **Read Confluence page**
and replace `PAGE_ID` with that ID or URL, keeping it as one JSON array element:

```json
["confluence", "content", "get", "PAGE_ID", "--detail", "full", "--format", "md", "--include-metadata", "-o", "json"]
```

Select the app language, start an interactive run, and approve the page read. Open
**Run files → `output/confluence-sdlc-brief.md`** when completed. The report records page
metadata when returned, documented requirements and decisions, source references,
risks, questions and proposed tests. It follows the exact native output file reference;
if TWG returns a separate body file, it uses that explicit reference too. Missing or
truncated content remains a gap. It does not edit Confluence or execute the proposed tests.

A 404 from the Confluence site or API requires checking product activation, the selected
site, page existence and access. Do not treat an account connection test as proof that
Confluence is provisioned, or substitute a fictitious page ID to produce a successful report.

## 10. Interpret the results and limits

**Builds:** empty build statuses mean no status was returned, not that CI passed. A sample
repository without a configured pipeline may legitimately have no builds. These workflows
read available statuses; they do not configure or run pipelines.

**Issue–PR linkage:** putting an issue key in a PR title, branch, commit message or description
provides a textual reference. The template does not query Jira's development panel or
remote links to verify a formal association. It must distinguish an observed reference
from an unverified structured link. This is a current check limitation, not proof that
authentication failed or that no association exists.

**Diff evidence:** file statistics describe paths and line counts, not the content or quality
of the change. The dossier does not claim to review the actual unified diff. The request
limit is 100 files; returned pagination, truncation or coverage gaps must remain explicit.
A response without coverage metadata is not proof that all files were acquired.

**Requirements:** a placeholder issue with an empty description cannot establish a functional
requirement baseline. The report should record this gap rather than invent intent.

**Language:** all three templates are localized in English and Italian. New reports follow the
app language selected at run start, including when using a copy originally created in
the other language. Changing the UI afterward does not translate historical reports.

## 11. Troubleshooting

| Symptom | Check / next step |
|---|---|
| OAuth login fails after authorization | Include account/profile read access as well as the needed product scopes, then reconnect. |
| Verification code expired | Request a new code on Atlassian and enter it directly there. If resend fails, retry the sign-in flow later. |
| Site validation fails | Use an HTTPS `*.atlassian.net` site root without paths, queries or embedded credentials. |
| Atlassian connects but Jira or Confluence read fails | Product permissions, account access and the specific issue/page identifier. |
| Bitbucket token not configured | Save the token and account email in the same local app account used for the run. |
| Bitbucket connection not verified | Token expiration/revocation, scope selection, matching email and repository access. |
| Connection test succeeds but PR read fails | Check the repository and PR arguments in both workflow nodes; the settings test does not read that PR. |
| Workflow waits for review | Open **Review required** and inspect the pending native command. |
| Native TWG command fails | Check login, permissions, command syntax and target. Raw authentication diagnostics are not exposed by the workflow. |
| Capture task reports blocked | Check that the immediately preceding native node supplied an actual JSON file. Do not substitute another run's evidence. |
| Snapshot verification fails | Start a fresh run and preserve the original evidence; do not edit snapshots to make verification pass. |
| Diff statistics missing | Confirm the separate `diffstat` node exists, uses the intended PR/repository and completed successfully. |
| Old personal copy still uses the previous graph | Create a fresh copy from the updated catalog, or carefully adopt all capture tasks, helper files and the separate diffstat node. |

Keep credentials out of diagnostic reports. Historical runs remain unchanged when you
update a template or create a new personal copy.

## Recheck existing personal workflows

The catalog and personal workflow copies are separate. Updating a published template
does not alter existing copies or historical run snapshots. Before a regression run:

1. Compare the copy with its current template, preserving the actual issue/page IDs
   and repository/PR bindings. Keep credentials in Personal settings.
2. For the combined workflow, confirm the Jira and PR detail nodes, the separate
   `diffstat` read, all three evidence-capture tasks, and final verification are present.
   Both Bitbucket nodes must target the same workspace/repository and PR number.
3. Keep the bundled `examples/atlassian` Source in copy mode so the current
   `capture_evidence.py` helper is available. Do not replace it with files from an old run.
4. Start a fresh run after selecting the report language. Inspect and approve each
   intended native read when requested; these approvals do not approve the remote PR.
5. Require a completed run and a newly generated report. For the dossier, verify that
   the three snapshots match their originating native files and provenance manifests.

Recheck Jira, the combined Jira + Bitbucket dossier and Confluence independently. A
working account connection or successful Jira run does not certify the other targets.
Empty issue requirements, missing build statuses, unverified structured issue–PR linkage
and Confluence conversion gaps remain findings in the report; they must not be replaced
with invented evidence to make the demonstration appear complete.
