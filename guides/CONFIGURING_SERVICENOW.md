# Configuring the ServiceNow workflows

This guide covers ServiceNow connectivity, ITSM classification, catalog Requests and
Spec Kit examples. Start from a built-in template and keep business-specific settings in
files inside its Source.
Personal Settings hold the ServiceNow connection and Pi agent configuration.

| Template in New workflow | Purpose | Bundled Source |
|---|---|---|
| **ServiceNow · OAuth connection test** | Confirm OAuth Client Credentials and incident read access, then write a verification report. | `examples/servicenow` |
| **Automate ITSM ticket classification and resolution** | Classify a batch using Pi and internal RAG, prepare a plan for every ticket, then apply approved actions. | `examples/itsm` |
| **Automate ITSM ticket classification and resolution · Northstar KB demo** | The same pipeline with four sample KB articles covering VPN, software installation, password resets and ticket status. | `examples/itsm` |
| **ServiceNow · Requests from configuration files** | Order catalog items from explicit routes and per-request variables. | `examples/servicenow` |
| **ServiceNow · Software requirement to Spec Kit** | Read a request or software requirement and produce a traceable specification for human review. | `examples/servicenow` |

The ITSM workflow classifies input tickets; it does not poll ServiceNow for incoming
incidents. The Requests workflow starts from requests already identified as catalog
orders. The Spec Kit workflow reads an existing record and performs no ServiceNow writes.

## 1. Create a workflow from a built-in template

1. Click **New workflow**.
2. Find the templates under **ServiceNow and ITSM** and select one of the names above.
3. Confirm or change the name of your personal copy, then save it.
4. Configure Pi and ServiceNow as described below. For ITSM, also bind an internal RAG index.
5. Use the bundled Source for a demonstration, or point its generic Source node to your
   own configuration folder before starting a new run.

No workflow-generation command or JSON import is needed. The templates include the nodes,
connections, prompts and relative configuration paths. Existing personal workflows and
historical runs remain separate from the template catalog. An installation with a customized
saved catalog can add these templates through the administrator's template manager without
replacing its other entries.

## 2. Configure the PDI and OAuth

### 2.1 Prepare the instance and integration user

Activate your Personal Developer Instance from the ServiceNow Developer portal and open
the instance URL, such as `https://devNNNN.service-now.com`. Use an instance administrator
for the one-time OAuth setup. Developer portal credentials and instance credentials are
separate.

Create or reuse a dedicated integration user, for example `rightcommit.api`. Keep it active
and assign only the roles/ACL permissions required by the workflow. The OAuth connectivity
test needs permission to read incidents. An existing integration user that can already
read incidents through Basic authentication is a useful baseline, but OAuth scope restrictions
still apply. The integration user does not need admin merely to run this read-only test.

For the examples, distinguish the required permissions:

| Operation | API/table access to authorize |
|---|---|
| Connection check | Table API `GET`, table `incident`. |
| Read requirement | Table API `GET` for the configured table, usually `sc_req_item`. |
| Read standard RITM variables | Table API `GET` for `sc_item_option_mtom` and referenced fields. |
| Create incident | Table API `POST` on `incident`, plus applicable ACLs. |
| Resolve/update incident | Table API `PATCH` on the selected incident, plus record conditions and ACLs. |
| Order catalog item | Service Catalog ordering endpoint and access to the specific item and variables. |

Scopes authorize API/method access; they do not replace table, field or record ACLs.
See [ServiceNow REST API access controls](https://www.servicenow.com/docs/r/api-reference/rest-api-explorer/c_RESTAPI.html).

### 2.2 Enable inbound Client Credentials

In the instance's system properties, verify or create:

| Property field | Value |
|---|---|
| Name | `glide.oauth.inbound.client.credential.grant_type.enabled` |
| Type | Boolean / true-false |
| Value | `true` |

The property enables the inbound OAuth Client Credentials grant. See
[ServiceNow's Client Credentials property setup](https://www.servicenow.com/docs/r/platform-security/authentication/create-cc-sys-prop.html).

The instance must have OAuth, REST API and authentication-scope functionality available.
If the related scope configuration menus are missing, check the required plugins and
administrator roles against the documentation for your instance release.

### 2.3 Register the OAuth application

Open **System OAuth → Application Registry → New**. In the newer interface, choose
**New Inbound Integration Experience → New Integration → OAuth Client Credentials Grant**.
Depending on the release, the integration may also appear under **Machine Identity Console
→ Inbound integrations**. In a legacy registry, choose an inbound endpoint for external
clients and configure the Client Credentials grant.

Set:

| Field | Example/configuration |
|---|---|
| Name | `rightcommit OAuth demo` |
| Provider Name, when required | `rightcommit` |
| Grant type | Client Credentials |
| OAuth application user | The dedicated integration user, such as `rightcommit.api`. |
| Client ID | Generated by ServiceNow; copy it from the saved application. |
| Client Secret | Generated/configured by ServiceNow; keep it private. |

Client ID and Client Secret are application credentials, not the user's password or an
AI provider key. This grant uses the token endpoint directly; it does not need a browser
redirect flow in rightcommit. See
[ServiceNow OAuth Client Credentials configuration](https://www.servicenow.com/docs/r/platform-security/authentication/configure-an-oauth-client-credential-grant.html).

### 2.4 Create the read scope and map it to the Table API

Use a dedicated scope named **`rightcommit_table_read`** for the read-only test.

1. Open **API Auth Scopes → REST API Auth Scope**; the module may be under
   **System Web Services**.
2. Create a configuration, for example named `rightcommit Table API read`, and enable **Active**.
3. Select **REST API = Table API**. The API path is `now/table`.
4. In **Auth Scope**, create or select the scope whose actual token-request value is
   `rightcommit_table_read`. A configuration record's display name is not a substitute
   for this scope value.
5. Disable **Apply auth scope to all HTTP methods in this API** and select **GET**.
6. Apply it to the API versions/resources used by the example. The example uses the
   unversioned Table API incident-list resource. Do not restrict the mapping to a different
   version or to only a single-record resource.
7. Save.

[ServiceNow's REST API Auth Scope configuration](https://www.servicenow.com/docs/r/platform-security/authentication/configure-rest-api-auth-scope.html)
describes the API, method, version and resource selectors.

**Also associate that same Auth Scope with the OAuth application.** Return to the
application created in §2.3 and add it in **Auth Scopes** or **Scope validation settings**,
using the existing scope record rather than creating another scope with a similar label.
Set it as default if the interface provides that option, then save.

There are three necessary links:

```text
OAuth application ── allows ── rightcommit_table_read
                                  │
REST API Auth Scope ── maps ────────┘ ── to Table API GET
                                  │
rightcommit OAuth Scope field ── requests this exact value
```

Creating the REST API mapping alone is insufficient. Likewise, typing the scope name in
rightcommit does not grant it to the application. Keep API scope restrictions and user ACLs
in place and authorize the API needed for the test.

If **Enforce token restriction** is enabled, also check the corresponding REST API Access
Policies: they must allow the intended OAuth client/API. That setting is separate from the
scope-to-API mapping.

### 2.5 Configure rightcommit Personal Settings

Open **Personal Settings → Connections → ServiceNow**:

| Field | Value |
|---|---|
| ServiceNow instance URL | Your instance origin, e.g. `https://devNNNN.service-now.com`; no `/api` suffix. |
| Authentication | **OAuth · Client Credentials** |
| OAuth → Client ID | The application's Client ID. |
| OAuth → Client Secret | The application's Client Secret. |
| OAuth → Scope | `rightcommit_table_read` for this test. Multiple scopes are space-separated when supported/configured on the PDI. |

Click **Test connection**. It uses the current fields without saving them and performs
an incident read, so successful token issuance alone does not make the check pass.
After a successful check, click **Save settings**: workflow runs use saved settings.

rightcommit obtains tokens from the instance's `/oauth_token.do` endpoint, keeps them in
memory and requests a replacement before expiry. Concurrent calls share token acquisition.
Client Credentials uses a new token request rather than a refresh-token flow. A local
service restart discards the cache; the next request obtains a token again. This also
supports scheduled/unattended workflows without manually pasting expiring access tokens.
The token lifetime is chosen by ServiceNow, not hardcoded by the workflow.

Client Secret is stored encrypted, masked in settings responses and excluded from agent
context. Leave its field blank to keep the saved secret for the same instance/Client ID;
use the trash button to remove it. Changing the instance or Client ID requires the
corresponding secret again.

**Basic** remains available for username/password authentication. **Manual bearer token**
is available for externally obtained tokens and does not renew them automatically.
A rejected automatic token is invalidated; reads may retry once after renewal. Writes
are never automatically replayed following an authentication error.

In **Personal Settings → Agents**, configure Pi, including provider, model and credentials,
for the report task. With multiple providers, select Multi-provider and configure keys in
the environment. Native startup loads `.env` from the startup directory; existing process
variables win. Docker must pass the variables into the container, as the supplied Compose
configuration does. ServiceNow credentials belong in Personal Settings, not in `.env`,
Source files, workflow JSON or prompts for these examples.

### 2.6 Run the OAuth test template

In **New workflow → ServiceNow and ITSM**, choose:

- English: **ServiceNow · OAuth connection test**.
- Italian: **ServiceNow · Test connessione OAuth**.

The selected UI language determines the new copy's title, node names, setup notes and
report language. The original personal workflow and its historical runs remain unchanged.
No workflow-generation script, manual import or catalog-item configuration is required.

The graph performs two operations:

1. **Native connection check**: uses the saved ServiceNow settings, automatically obtains
   an OAuth token as needed, and reads at most one incident through the Table API. Its
   result records the actual authentication method and instance.
2. **Pi report**: writes `output/oauth-check.md` from native evidence. It confirms OAuth
   only when the recorded method is `client_credentials`. Basic/manual bearer or missing
   authentication evidence is explicitly reported as OAuth not confirmed and blocks the task.

The report identifies missing metadata rather than inventing HTTP codes, identities or
check timestamps. A report creation timestamp is distinct from the check timestamp.
If the native node fails, the run stops there; a Pi report may not exist. Inspect the
native node error in that case.

A successful run confirms OAuth Client Credentials plus incident read access. It does
not prove catalog ordering, write/resolve permissions, or that token expiry was exercised
by that short run. The test creates, updates and resolves no ServiceNow records.

For later write workflows, configure separate appropriate scopes/API mappings and the
required user ACLs. The GET-only `rightcommit_table_read` scope is deliberately insufficient
for POST/PATCH/catalog orders.

### 2.7 OAuth errors observed during the PDI test

| Evidence/error | Meaning and next check |
|---|---|
| Token endpoint HTTP 200, incident GET HTTP 403 | Authentication credentials can obtain a token; API authorization still fails. Check granted scopes, mapping, application user and ACLs. |
| `Access to unscoped api is not allowed` | The restricted OAuth client is calling an API without the required applicable scope mapping. Check the Table API mapping and its method/version/resource. |
| `Missing required api access scope: rightcommit_table_read` | The API requires the scope but the token lacks it. Associate that same scope with the OAuth application and request its exact value in rightcommit. |
| Requested scope present, returned token scope empty | The request value alone is not authorization. Check the application's allowed/default scope association and the actual scope value. |
| Token endpoint HTTP 400/401 | Check application credentials, grant type, enabled system property and OAuth application user. |
| Scope granted but incident GET still denied | Check API Access Policies and the application user's table/field/record ACLs. A scope does not grant user roles. |
| Test succeeds with Basic but fails with OAuth | Compare OAuth scope restrictions and its mapped application user; successful Basic access does not prove OAuth authorization. |
| Python helper certificate error on macOS | A separate Python trust-store issue, not proof that OAuth credentials are invalid. The built-in rightcommit connection avoids the manual Python-token step. |

After changing PDI scope policies, test with a newly issued token. A previously cached token
may retain its old grants until renewed. Complete the PDI configuration before the first
check; if an old token persists, allow it to expire or restart the local service when no
runs are executing. Do not rotate Client Secret merely to work around missing API grants.

The successful live test used an automatically obtained token with scope
`rightcommit_table_read`, received HTTP 200 for incident reading, and produced a native
`authentication: client_credentials` result plus `output/oauth-check.md`. No writes were
performed. Instance identifiers, client credentials and access tokens are intentionally
absent from the portable examples.

## 3. Configure files in a personal Source folder

Templates initially use a copied Source of type **Folder in examples**:

- `itsm` for classification and its Northstar demo;
- `servicenow` for OAuth verification, catalog Requests and Spec Kit.

For repeatable live use, copy the entire relevant example into the application's **sources**
root. Include hidden `.pi` and `.specify` directories when copying the Spec Kit example.

### Native execution from the repository

```sh
mkdir -p sources/itsm-demo sources/servicenow-demo
cp -R examples/itsm/. sources/itsm-demo/
cp -R examples/servicenow/. sources/servicenow-demo/
```

If `NODO_SOURCES` points elsewhere, use that root instead. Edit the files in your personal
copy rather than the distribution or an extracted assets cache.

### The project's test container

The `/sources` volume is writable even when the container root filesystem is read-only:

```sh
docker exec rightcommit-itsm-test mkdir -p /sources/itsm-demo /sources/servicenow-demo
COPYFILE_DISABLE=1 tar --no-xattrs -C examples/itsm -cf - . | docker exec -i rightcommit-itsm-test tar -C /sources/itsm-demo -xf -
COPYFILE_DISABLE=1 tar --no-xattrs -C examples/servicenow -cf - . | docker exec -i rightcommit-itsm-test tar -C /sources/servicenow-demo -xf -
```

These copy commands are for macOS; on systems without `--no-xattrs`, omit that option.
Prepare your JSON locally before copying, then copy updated files again when necessary.
For other installations, replace the container name or mount a personal folder under the
application's sources root.

### Connect the template to your folder

Open the Source node in your personal workflow, select **Folder in /sources**, choose
`itsm-demo` or `servicenow-demo`, and keep **copy** mode. Save the workflow. This is the
standard Source configuration; no catalog-specific UI fields are needed.

Each run receives its own Source copy. Changing a Source folder affects subsequent runs,
not the files already copied into an existing run. Avoid copying previous run outputs
back into the input folder.

## 4. ITSM ticket classification and resolution

### Files and preparation

| File under `examples/itsm` | Purpose |
|---|---|
| `tickets/batch.json` | Stable namespace and 1–100 input tickets with unique request IDs. |
| `kb/*.md` | Four sample articles for the internal RAG index. |
| `config/servicenow.json` | Catalog routes, item IDs and variable mappings for Request tickets. |
| `itsm.py` | Validate classifications, generate individual/batch plans and prepare verified resolution data. |
| `test_itsm.py` | Deterministic local contract tests. |
| `workflow-generic.json`, `workflow-northstar.json` | Source copies of the built-in template graphs. |

1. Create the generic or Northstar workflow from **New workflow**.
2. In **Storage**, index `examples/itsm/kb`, or your own KB under sources. Keyword mode
   avoids a model download; hybrid mode adds semantic retrieval with the configured model.
3. Add a **RAG** node, select that personal index and connect its context output to
   **Pi ITSM operator**. Personal index IDs are deliberately absent from portable templates.
4. Edit `tickets/batch.json` in your Source. Keep a stable namespace and request_id for
   each ticket. Complete the Request routes in `config/servicenow.json` for real orders.
5. Save and start the workflow in interactive mode.

For a real customer, replace the demo KB, ticket manifest and catalog mapping. The generic
and Northstar templates use the same demonstration files as a starting point.

### What happens in a run

The connection check verifies access first. Pi then checks the RAG binding, reads every
ticket, classifies it as **Incident**, **Request** or **Other**, and searches the internal
RAG index separately for each ticket. It preserves the actual index generation and
relevant source chunks in `output/batch-analysis.json`.

The local Python helper validates the analysis and prepares the plans:

```sh
python3 itsm.py batch --batch tickets/batch.json --analyses output/batch-analysis.json --config config/servicenow.json --out output/batch-plan.json
```

It creates `output/batch-plan.json` and `output/plans/<request_id>.json`. Python performs
validation and file preparation; the native ServiceNow connector makes the API calls.
After approval of the exact batch content, the connector processes every item:

- **Incident:** create an incident or reuse its recorded result.
- **Request:** resolve its catalog route and variables from the configuration, then order
  the item or reuse its recorded result.
- **Other:** answer from relevant KB evidence. Without suitable evidence, return
  `needs_human` for service-desk follow-up.

One blocked item does not prevent processing the others. The receipt is
`output/servicenow-apply-plan.json`. The final report lists each request ID, classification,
action, status and actual ServiceNow number/sys_id when available. Possible statuses are
`applied`, `reused`, `answered`, `needs_human`, `blocked` and `attention_required`.
A technically completed run does not mean every item succeeded.

A missing plan error names the required file, such as
`File piano mancante: output/batch-plan.json`. Check the preceding task's actual output
and the Source copied into that run.

### Optional verified incident resolution

KB retrieval and incident creation do not prove that a technical problem is fixed.
To test resolution, explicitly select **one Incident request_id** in the run input and
request the human resolution review. Provide concrete user verification evidence; label
simulated PDI checks as simulations. Request/Other items, blocked entries and incidents
already resolved or closed do not trigger a new resolution.

After the human review, Pi prepares `output/resolution-evidence.txt` and runs:

```sh
python3 itsm.py resolution --created output/servicenow-apply-plan.json --request-id demo-incident-001 --evidence-file output/resolution-evidence.txt --out output/resolution.json
```

The helper selects the confirmed Incident from the native batch receipt. The connector
checks its request ID, namespace, sys_id, number, run and creation node before approval
and before sending the PATCH. It rejects unrelated incidents, including the record used
only for the connection check. The exact update requires approval; ServiceNow must confirm
state `6`. The result is saved to `output/servicenow-resolve-ticket.json`.

## 5. Catalog Requests from configuration files

This separate workflow does not classify free-text tickets. It reads `requests.json`,
prepares explicit Request plans and applies them using `config/catalog-routing.json`.

### Select real catalog items and internal variable names

Find an item orderable by the PDI integration user. Copy its `sys_id` from the record/URL.
Inspect its variable definitions and use **Name**, the internal name, rather than the
label shown to the requester. Record required values, available choices and reference
identifiers. A user reference may need the user's sys_id rather than their display name.

Example route in `config/catalog-routing.json`:

```json
{
  "schema": "rightcommit.servicenow.catalog.v1",
  "namespace": "software-requests-demo",
  "catalog": {
    "software_installation": {
      "item_sys_id": "",
      "variables": {
        "software_name": {"from": "/fields/software_name"},
        "software_version": {"from": "/fields/version"},
        "business_justification": {"from": "/fields/justification"}
      },
      "required_variables": ["software_name", "business_justification"]
    }
  }
}
```

Replace the blank item ID and adapt variable keys to your item. `from` reads a per-request
value through a JSON pointer; `literal` supplies a constant, such as `{"literal": 1}`.
Scripts, expressions and environment substitutions are not evaluated. The namespace
identifies the source flow/system and must match the request manifest.

Each entry in `requests.json` selects a route and supplies its own fields:

```json
{
  "namespace": "software-requests-demo",
  "tickets": [
    {
      "request_id": "software-python-001",
      "description": "Install Python for the development team",
      "catalog_route": "software_installation",
      "fields": {
        "software_name": "Python",
        "version": "3.12",
        "justification": "Development work"
      }
    }
  ]
}
```

The ITSM workflow uses the same catalog configuration schema in `config/servicenow.json`.
Its manifest is `tickets/batch.json` and includes classification text alongside route/fields.

### Prepare and run

Optionally check plan preparation locally from the Source folder:

```sh
python3 request_batch.py --config config/catalog-routing.json --tickets requests.json --out output/request-plans.json
```

Create **ServiceNow · Requests from configuration files** from the template picker and
start it in interactive mode. Review the per-request payloads before approving the batch.
A missing item, unknown route or missing required value blocks that Request; other
entries continue. The connector does not infer routes or invent item IDs.

### Deduplication for both write workflows

Keep the same namespace and request_id when retrying the same ticket. The logical identity
is **ServiceNow instance + namespace + request_id**, independent of the run ID. Encrypted
per-user receipts and atomic claims prevent repeat/concurrent submissions in the same app.
An already applied order returns `reused`. Incident correlation IDs also support recovery
after a lost response; reused incidents are read again to report their current state.

An uncertain catalog-order outcome requires reconciliation and is never blindly resent.
Changing an already applied ticket's original text, category, item or variables causes a
conflict; use an explicit update workflow. Changing KB suggestions alone does not create
another ticket or overwrite its earlier notes. Assign a new ID only for a genuinely new
event. Preserve the app data volume because it contains deduplication receipts.

Legacy runs without stable identities require explicit reconciliation. Do not match old
records by title alone. Demo records already reconciled under `northstar-demo` must keep
that namespace when replaying the same input identities.

## 6. ServiceNow request → Spec Kit

### Files and fixture test

| File under `examples/servicenow` | Purpose |
|---|---|
| `config/request-to-spec.json` | Source selector and specification mapping; defaults to fixture mode. |
| `config/request-to-spec.live.example.json` | Live configuration to complete for your PDI. |
| `fixtures/software-requirement.json` | Explicitly synthetic software requirement. |
| `prepare_spec.py` | Normalize the snapshot, preserve provenance and verify generated files. |
| `.pi/prompts/`, `.specify/` | Official vendored Spec Kit commands and support files. |
| `SPECKIT_PROVENANCE.json`, `SPECKIT-LICENSE.txt` | Version/source hashes and license. |

Create **ServiceNow · Software requirement to Spec Kit** from **New workflow**. The default
`source.mode: fixture` reads the bundled sample without contacting ServiceNow or requiring
ServiceNow credentials. Pi is still required for the agent tasks.

The workflow runs the installed `/speckit.specify` command. The vendored integration is
Spec Kit 1.2.2; no runtime network installation is required for this example. It produces:

- `output/servicenow-read-request.json`: native snapshot with fixture/live provenance.
- `output/requirement-brief.md`: normalized input with source-field references.
- `output/source-requirement.json`: mapping, missing data and snapshot hash.
- `specs/001-software-approval-portal/spec.md`: draft specification.
- `specs/001-software-approval-portal/SOURCES.md`: provenance and requirement traceability.
- `specs/001-software-approval-portal/checklists/requirements.md`: quality checklist.

The feature directory follows the configured number and slug. The human review accepts
documentation only. It does not authorize implementation, deployment, ordering or
ServiceNow updates. Missing information remains `NEEDS CLARIFICATION`.

### Enable a live PDI read

Copy `config/request-to-spec.live.example.json` over `config/request-to-spec.json` in your
personal Source folder. Set `source.mode` to `servicenow` and complete **exactly one** selector:

- `source.sys_id`: the exact record ID; or
- `source.number`: the exact record number, which must resolve uniquely.

The default table is `sc_req_item`. `source.fields` lists fields to acquire. Connection
credentials always come from Personal Settings. With `include_variables: true`, RITM
variables are read through `sc_item_option_mtom`. With `variables_required: true`, denied
or incomplete variable acquisition fails. Setting it to false permits transport failures
with an explicit acquisition gap. Repeated variables/MRVS need a dedicated adapter.

For another requirement table installed on your PDI, configure table, selector and fields,
set `include_variables: false`, and update the mapping. Do not assume every module or
requirement table exists on every instance.

### Specification mapping and incomplete records

Under `specification`, `feature_number` and `feature_slug` choose the stable output folder.
Mapping entries are JSON pointers into the acquired snapshot:

```json
{
  "title": "/record/short_description",
  "description": "/record/description",
  "business_goal": "/variables/business_goal",
  "acceptance_criteria": "/variables/acceptance_criteria",
  "constraints": "/variables/constraints"
}
```

Title and description are required. Other missing inputs remain visible. `SOURCES.md`
must cite the exact source record and map requirements back to fields or variables.

Some PDI sample RITMs have empty descriptions and describe hardware orders rather than
software requirements. Do not turn them into invented software features. If catalog text
is useful for an acquisition test, explicitly include `cat_item.name` and
`cat_item.description` in `source.fields`, and map title/description to
`/record/cat_item.name` and `/record/cat_item.description`. Dot-walk field names remain
literal keys in the snapshot. Make clear that this text describes the catalog offering,
not requirements expressed by the requester. Empty variables do not establish completeness.
For a meaningful software-specification test, select a record containing actual software
needs, business goals and acceptance criteria.

After generation, the following check verifies artifact presence and source references;
it does not replace human validation of the business requirements:

```sh
python3 prepare_spec.py verify --config config/request-to-spec.json --snapshot output/servicenow-read-request.json
```

A run waiting at `review-spec` is awaiting a human decision. The final `verify-spec` task
runs after approval. Unresolved questions should be clarified before treating the draft
as ready for implementation.

For a real repository already using Spec Kit, use its own constitution and installed
commands. For a new Pi integration, initialize it with the selected CLI version using
`specify init --here --integration pi --script sh`. The example records the feature
folder in `.specify/feature.json` and does not publish branches or commits.

## 7. Checks and troubleshooting

From the repository:

```sh
python3 -m unittest discover -s examples/servicenow -p test_examples.py -v
python3 -m unittest discover -s examples/itsm -p test_itsm.py -v
cargo test servicenow -- --nocapture
python3 scripts/servicenow-examples-smoke.py
```

The smoke test uses a simulated Pi agent to verify fixture acquisition, graph execution,
Spec Kit command wiring, artifacts and review. It does not measure real model output
quality. Live validation requires an active PDI and Pi provider.

| Problem | Check |
|---|---|
| Template absent from New workflow | Check its published status and section in Manage templates; a personal workflow import does not add a catalog template. |
| Missing configuration/plan file | Use a path relative to the Source copied into the run, not the host repository. Inspect the preceding task output. |
| Request blocked | Check item_sys_id, catalog_route, internal variable names, required_variables and per-request values. |
| HTTP 401 | Check the instance URL and saved instance credentials, rather than Developer portal credentials. |
| HTTP 403 on record/variables/resolution | Check granted OAuth scopes and API mapping first, then table, field and relation ACLs and record conditions; see §2.7. |
| Record not found | Check table, sys_id/number, user visibility and unique number selection. |
| Missing Spec Kit command | Include `.pi` and `.specify`; keep Pi or install your chosen runtime's integration. |
| Missing title/description | Map to fields actually acquired; do not manufacture missing requirements. |
| Pi cannot write output | Native ServiceNow output must be owned by the isolated agent user; use the build containing the ownership fix. |
| Old configuration still used | Update the actual Source files and start a new run; existing runs keep their copies. |
| Uncertain order outcome | Reconcile in the PDI; do not delete receipts or change IDs to force a retry. |

## 8. References

- [ServiceNow inbound Client Credentials property](https://www.servicenow.com/docs/r/platform-security/authentication/create-cc-sys-prop.html).
- [ServiceNow OAuth Client Credentials configuration](https://www.servicenow.com/docs/r/platform-security/authentication/configure-an-oauth-client-credential-grant.html).
- [ServiceNow REST API Auth Scope configuration](https://www.servicenow.com/docs/r/platform-security/authentication/configure-rest-api-auth-scope.html).

- [ServiceNow REST APIs and access controls](https://www.servicenow.com/docs/r/api-reference/rest-api-explorer/c_RESTAPI.html).
- [ServiceNow Table API](https://developer.servicenow.com/print_page.do?category=null&identifier=c_TableAPI&module=api&release=zurich).
- [ServiceNow Service Catalog API](https://www.servicenow.com/docs/r/api-reference/rest-apis/c_ServiceCatalogAPI.html).
- [Spec Kit integrations and commands](https://github.github.com/spec-kit/integrations/index.html).
- [Spec Kit installation](https://github.github.io/spec-kit/installation.html).
