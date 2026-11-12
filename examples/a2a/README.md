# Local A2A incident demo

These two workflows demonstrate **Agent2Agent (A2A)** communication inside one
RightCommit installation, using the bundled Northstar incident `INC-1042`.
They use local example data: **no AI API key or external service is required**.

| File | Purpose |
| --- | --- |
| [`agent.json`](agent.json) | Reads the demo `incident.json` through a local Connection node and exposes its result through A2A. |
| [`client.json`](client.json) | Reads the demo incident and calls the published agent through an A2A node. |

## Run the pair

1. Start RightCommit locally and sign in. Use **Import workflow**
   (**Importa workflow**) to import both JSON files as separate workflows.
2. Open **A2A · 1 · Agente incidenti (demo)**. In the workflow toolbar,
   select **A2A**. Enable publication, select the incident result as a
   published output and publish the agent. **Copy the newly issued bearer
   token**: it is needed to call the agent.
3. Copy the published agent's **Agent Card URL** (the URL ending in
   `/.well-known/agent-card.json`). It must be reachable by the
   RightCommit instance executing the client workflow.
4. Open **A2A · 2 · Client incidenti (demo)** and select the **A2A**
   node, **Richiama agente incidenti**. Open its A2A connections editor;
   create a connection using the Agent Card URL and bearer token from
   steps 2–3. Save the connection.
5. In that node's **A2A connection** selector, select the **newly saved**
   connection, then use the available connection probe/test. Save the
   workflow before running it.
6. Select **Run workflow** (**Esegui workflow**) and confirm the run.
   The local step reads the demo incident, and the A2A step requests the
   published agent. Inspect the run details and output artifact for the
   `INC-1042` incident record.

## Important: the imported connection is not ready-made

`client.json` contains `"a2a_id": "a2a-demo-local"` as a sample
reference. **Importing the JSON does not create that A2A connection**.
You must explicitly create and select a real saved connection in step 4.
Do not assume the imported node's initial selection is valid.

Similarly, the comment inside `agent.json` describes an already
published agent, but a **fresh import must be published by its owner**
before it can be called.

## Troubleshooting

- **Missing or invalid A2A connection:** Select the newly saved
  connection on the client A2A node; the sample `a2a-demo-local` ID is
  not provisioned by import.
- **Unauthorized / token rejected:** Use the token issued for the
  published agent. If it was rotated, update the saved connection.
  Treat the token as a secret; do not commit it or paste it into issues.
- **Agent not reachable:** Verify the Agent Card URL from the
  executing application's network. A loopback URL points to the
  caller's own host, not a different server or container.
- **No incident artifact:** Confirm the agent workflow was published
  with the incident result selected as an exposed output, and inspect
  both workflows' run logs.

For maintainers, [`scripts/a2a-smoke.cjs`](../../scripts/a2a-smoke.cjs)
exercises publication, token issuance, Agent Card retrieval, saving an A2A
connection, probing and node persistence in a disposable test account.
It requires a running test instance and browser-test dependencies; it is
not a substitute for following the imported-example walkthrough above.
