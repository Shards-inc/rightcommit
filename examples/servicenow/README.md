# ServiceNow examples driven by configuration files

Create a workflow from **New workflow → ServiceNow and ITSM**, without generating
or importing workflow JSON. The templates use the bundled examples as a copied Source.
For personal configuration files, select a generic folder Source under /sources.

Three independent workflows, without catalog-specific forms in the application:

1. `workflow-oauth-check.json`: verify the saved OAuth Client Credentials connection by
   reading one incident, then produce `output/oauth-check.md` using native evidence. Configure
   `rightcommit_table_read` for Table API GET AND associate it with the OAuth application.
   The catalog localizes node text and reports in English or Italian.
2. `workflow-requests.json`: read `requests.json`, resolve each catalog route from
   `config/catalog-routing.json`, approve and apply the native Request batch.
3. `workflow-request-to-speckit.json`: acquire a requirement according to
   `config/request-to-spec.json`, normalize it and run the official Spec Kit specify command.

Credentials and the instance URL stay in Personal Settings → Connections → ServiceNow.
Catalog item sys_ids, internal variable names, request fields, source table/selector and
specification mapping belong in the files of this Source. Do not put passwords or tokens
in those files. Both workflows use a copied Source by default.

The catalog configuration intentionally has empty item_sys_id placeholders: complete them
with actual PDI items and internal variable names. Each Request chooses catalog_route and
supplies its own fields. The connector validates the configuration and shows the exact
per-request payload for approval. Missing data blocks that Request, while other entries
continue. A stable namespace/request_id and encrypted receipts prevent duplicate orders.
Unknown previous outcomes are not automatically resent.

The requirement workflow defaults to an explicitly labeled offline fixture, which does
not contact ServiceNow. To read the PDI, copy the settings from
`config/request-to-spec.live.example.json` to `config/request-to-spec.json` and set either
sys_id or number. GET-only acquisition reads sc_req_item plus its standard variables;
other tables can be read using configured fields and include_variables:false.
If variables are required and cannot be read, the workflow stops rather than inventing them.

Spec Kit 1.2.2 command assets are reused from the official vendored integration already
present in this repository; see SPECKIT_PROVENANCE.json and SPECKIT-LICENSE.txt.
No runtime network installation of Spec Kit is needed for the example. A Pi provider is
still needed for the agent steps. The specify node creates a specification and source
traceability, followed by human review. It does not implement software or write to ServiceNow.

Local preparation:

```sh
python3 request_batch.py
python3 prepare_spec.py prepare --snapshot fixtures/native-snapshot.example.json
python3 -m unittest discover -s . -p test_examples.py -v
```

The workflow itself produces output/servicenow-read-request.json; the fixture snapshot
above is only for an explicit offline command-line demonstration. Results are stored under
output/ and specs/<number>-<feature_slug>/. Nothing in output is shipped as a live result.

Activation, mounting/uploading a Source folder, PDI setup, real-world mapping and limitations:
[CONFIGURING_SERVICENOW.md](../../guides/CONFIGURING_SERVICENOW.md).
