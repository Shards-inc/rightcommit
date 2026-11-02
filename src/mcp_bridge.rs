//! Native stdio MCP facade. It cannot reach arbitrary servers: the per-turn grant is scoped by Rust.
use anyhow::{Context, Result};
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext,
};
use serde_json::{Value, json};
#[derive(Clone)]
struct Bridge {
    url: String,
    token: String,
    client: reqwest::Client,
}
impl Bridge {
    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> std::result::Result<T, McpError> {
        let res = self
            .client
            .post(&self.url)
            .bearer_auth(&self.token)
            .header("x-nodo-request", "1")
            .json(&json!({"method":method,"params":params}))
            .send()
            .await
            .map_err(|_| McpError::internal_error("rightcommit MCP bridge unavailable", None))?;
        if !res.status().is_success() {
            return Err(McpError::internal_error(
                "MCP call failed or grant expired; inspect the run log",
                None,
            ));
        }
        res.json()
            .await
            .map_err(|_| McpError::internal_error("Invalid MCP bridge response", None))
    }
}
impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        self.request("tools/list", Value::Null).await
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, McpError> {
        self.request::<CallToolResult>("tools/call", serde_json::to_value(params).unwrap())
            .await
            .map(Into::into)
    }
}
pub async fn run() -> Result<()> {
    let url = std::env::var("NODO_MCP_ENDPOINT").context("Missing MCP bridge endpoint")?;
    let u = reqwest::Url::parse(&url)?;
    anyhow::ensure!(
        u.scheme() == "http" && u.host_str() == Some("127.0.0.1") && u.path() == "/api/mcp/agent",
        "Invalid MCP bridge endpoint"
    );
    let bridge = Bridge {
        url,
        token: std::env::var("NODO_MCP_TOKEN").context("Missing MCP bridge grant")?,
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(330))
            .build()?,
    };
    let service = bridge.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_client::tests::fixture_app;
    use rmcp::model::ClientConfig;
    #[tokio::test]
    async fn native_stdio_facade_routes_real_rmcp_calls_through_scoped_http_grant() {
        let dir = tempfile::tempdir().unwrap();
        let (app, run) = fixture_app(dir.path()).await;
        let prepared = crate::mcp_client::prepare(&app, 1, &run)
            .await
            .unwrap()
            .unwrap();
        app.busy.lock().unwrap().insert(run.id.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/api/mcp/agent", listener.local_addr().unwrap());
        let router = crate::api::router(app.clone());
        let http = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let bridge = Bridge {
            url: endpoint,
            token: prepared.token().into(),
            client: reqwest::Client::new(),
        };
        let (server_io, client_io) = tokio::io::duplex(65536);
        let (sr, sw) = tokio::io::split(server_io);
        let (cr, cw) = tokio::io::split(client_io);
        let (server, client) = tokio::join!(
            bridge.serve((sr, sw)),
            ClientConfig::default().serve((cr, cw))
        );
        let server = server.unwrap();
        let client = client.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        let result = client
            .call_tool(
                CallToolRequestParams::new(tools[0].name.clone()).with_arguments(
                    serde_json::from_value(json!({"value":"bridge result"})).unwrap(),
                ),
            )
            .await
            .unwrap();
        assert!(
            serde_json::to_string(&result)
                .unwrap()
                .contains("bridge result")
        );
        drop(prepared);
        assert!(
            client
                .call_tool(CallToolRequestParams::new(tools[0].name.clone()))
                .await
                .is_err()
        );
        client.cancel().await.unwrap();
        server.waiting().await.unwrap();
        http.abort();
    }
}
