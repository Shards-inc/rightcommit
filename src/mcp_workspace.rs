//! Bundled, read-only example MCP server. It runs under the same UID as the agent.
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext,
};
use serde_json::json;
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
#[derive(Clone)]
pub struct Workspace {
    root: Arc<PathBuf>,
}
impl Workspace {
    fn path(&self, value: &str) -> anyhow::Result<PathBuf> {
        let path = Path::new(value);
        anyhow::ensure!(
            !path.is_absolute()
                && !path.components().any(|c| matches!(c, Component::ParentDir)
                    || [".git", ".nodo", ".rightcommit"].iter().any(|name| c.as_os_str() == *name)),
            "Percorso non consentito"
        );
        let p = std::fs::canonicalize(self.root.join(path))?;
        anyhow::ensure!(
            p.starts_with(self.root.as_path()),
            "Percorso fuori workspace"
        );
        Ok(p)
    }
}
impl ServerHandler for Workspace {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("nodo-workspace", "1.0"))
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let schema = json!({"type":"object","properties":{"path":{"type":"string","description":"Relative path within the run workspace"}}});
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new(
                "list_files",
                "List files in a workspace folder",
                schema.as_object().unwrap().clone(),
            ),
            Tool::new(
                "read_file",
                "Read a UTF-8 workspace file (max 256 KiB)",
                schema.as_object().unwrap().clone(),
            ),
        ]))
    }
    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let result = (|| -> anyhow::Result<String> {
            let value = req
                .arguments
                .as_ref()
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or(".");
            let path = self.path(value)?;
            match req.name.as_ref() {
                "list_files" => {
                    let mut names = std::fs::read_dir(path)?
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            !e.path().is_symlink()
                                && e.file_name() != ".git"
                                && e.file_name() != ".nodo"
                                && e.file_name() != ".rightcommit"
                        })
                        .take(1000)
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect::<Vec<_>>();
                    names.sort();
                    Ok(serde_json::to_string(&names)?)
                }
                "read_file" => {
                    anyhow::ensure!(path.metadata()?.len() <= 256 * 1024, "File troppo grande");
                    Ok(std::fs::read_to_string(path)?)
                }
                _ => anyhow::bail!("Tool sconosciuto"),
            }
        })();
        Ok(match result {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(_) => CallToolResult::error(vec![ContentBlock::text(
                "Workspace MCP: file o percorso non disponibile",
            )]),
        }
        .into())
    }
}
pub async fn run() -> anyhow::Result<()> {
    let server = Workspace {
        root: Arc::new(std::fs::canonicalize(std::env::current_dir()?)?),
    }
    .serve(rmcp::transport::stdio())
    .await?;
    server.waiting().await?;
    Ok(())
}
