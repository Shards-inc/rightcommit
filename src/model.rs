use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn default_runtime() -> String {
    "codex".into()
}

pub const NODE_KINDS: &[&str] = &[
    "Source",
    "Agent",
    "Task",
    "Fork",
    "Join",
    "Review",
    "Gate",
    "Memory",
    "Connection",
    "Workflow",
    "MCP",
    "RAG",
    "Comment",
    "A2A",
];

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub markdown: bool,
    pub title: String,
    pub x: f64,
    pub y: f64,
    pub phase: String,
    pub prompt: String,
    pub command: String,
    pub skip: bool,
    pub agent: String,
    pub runtime: String,
    pub model: String,
    pub pi_provider: String,
    pub pi_base_url: String,
    pub mode: String,
    pub location: String,
    pub source_type: String,
    pub source_mode: String,
    pub target: String,
    pub workflow_id: String,
    pub rag_index: String,
    pub rag_mode: String,
    pub rag_top_k: usize,
    pub a2a_id: String,
    pub mcp_id: String,
    pub mcp_tools: Vec<String>,
    pub provider: String,
    pub organization: String,
    pub project: String,
    pub repository: String,
    pub operation: String,
    pub api_endpoint: String,
    pub api_method: String,
    pub api_path: String,
    pub api_parameters: String,
    pub api_query: String,
    pub api_headers: String,
    pub api_body: String,
    pub api_encoding: String,
    pub outcomes: Vec<String>,
    pub require_tests: bool,
    pub require_tasks_closed: bool,
    pub max_rework: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: String,
    #[serde(default)]
    pub outcome: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    #[default]
    Interactive,
    Assisted,
    Unattended,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ExecutionPolicy {
    pub mode: ExecutionMode,
    pub runtime: String,
    pub model: String,
    pub instructions: String,
    pub max_decisions: u32,
    pub max_per_node: u32,
    pub allow_external_writes: bool,
}
impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Interactive,
            runtime: String::new(),
            model: String::new(),
            instructions: String::new(),
            max_decisions: 50,
            max_per_node: 5,
            allow_external_writes: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupervisorProposal {
    pub action: String,
    pub answer: String,
    pub rationale: String,
    pub session: String,
    pub turn: u32,
    pub created: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(default)]
    pub execution: ExecutionPolicy,
    #[serde(default)]
    pub created: i64,
    pub id: String,
    pub name: String,
    pub description: String,
    pub entry: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}
impl Workflow {
    pub fn node(&self, id: &str) -> anyhow::Result<&Node> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| anyhow::anyhow!("Nodo inesistente: {id}"))
    }
    pub fn next(&self, id: &str, outcome: &str) -> Option<&Edge> {
        self.edges.iter().find(|e| {
            e.from == id && (e.kind == "control" || e.kind == "rework") && e.outcome == outcome
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub multi_provider: bool,
    pub base_url: String,
    pub model: String,
    pub effort: String,
    pub api_key: String,
    pub provider: String,
    /// Pi only: declare the built-in `codemode` tool in Task turns.
    pub codemode: bool,
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            multi_provider: false,
            base_url: String::new(),
            model: String::new(),
            effort: "low".into(),
            api_key: String::new(),
            provider: "anthropic".into(),
            codemode: false,
        }
    }
}
/// Zero uses the native model/provider default, except Pi's explicit token budgets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionConfig {
    pub context_window_tokens: u64,
    pub threshold_tokens: u64,
    pub trigger_percent: u8,
    pub reserve_tokens: u64,
    pub keep_recent_tokens: u64,
}
impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            context_window_tokens: 0,
            threshold_tokens: 0,
            trigger_percent: 0,
            reserve_tokens: 16384,
            keep_recent_tokens: 20000,
        }
    }
}
impl CompactionConfig {
    pub fn for_runtime(runtime: &str) -> Self {
        if runtime == "claude" {
            Self {
                trigger_percent: 80,
                ..Self::default()
            }
        } else {
            Self::default()
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub manager_write_enabled: bool,
    pub default_runtime: String,
    pub agents: BTreeMap<String, AgentConfig>,
    pub connections: BTreeMap<String, ConnectionConfig>,
    pub mcp_servers: BTreeMap<String, McpConfig>,
    pub task_timeout_secs: u64,
    pub source_copy_limit_mib: u64,
    pub file_preview_limit_mb: u64,
    pub workflow_parallelism: usize,
    pub activity_log_verbosity: String,
    pub activity_log_page_size: usize,
    pub compaction: BTreeMap<String, CompactionConfig>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            manager_write_enabled: true,
            default_runtime: default_runtime(),
            agents: ["codex", "claude", "pi"]
                .into_iter()
                .map(|x| (x.into(), AgentConfig::default()))
                .collect(),
            connections: BTreeMap::new(),
            mcp_servers: BTreeMap::new(),
            task_timeout_secs: 7200,
            source_copy_limit_mib: 10,
            file_preview_limit_mb: 5,
            workflow_parallelism: 5,
            activity_log_verbosity: "preview".into(),
            activity_log_page_size: 10,
            compaction: ["claude", "codex", "pi"]
                .into_iter()
                .map(|rt| (rt.into(), CompactionConfig::for_runtime(rt)))
                .collect(),
        }
    }
}
impl Settings {
    pub fn file_preview_limit_bytes(&self) -> anyhow::Result<u64> {
        anyhow::ensure!((1..=100).contains(&self.file_preview_limit_mb), "Limite anteprima file: inserisci un valore da 1 a 100 MB");
        Ok(self.file_preview_limit_mb * 1_000_000)
    }
    pub fn source_copy_limit_bytes(&self) -> anyhow::Result<u64> {
        anyhow::ensure!(
            self.source_copy_limit_mib > 0,
            "Il limite copia sorgenti deve essere almeno 1 MiB"
        );
        self.source_copy_limit_mib
            .checked_mul(1024 * 1024)
            .ok_or_else(|| anyhow::anyhow!("Limite copia sorgenti troppo grande"))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConnectionConfig {
    pub authentication: String,
    pub client_id: String,
    pub client_secret: String,
    pub oauth_scope: String,
    pub username: String,
    pub password: String,
    pub catalog_item: String,
    pub catalog_variables: String,
    pub base_url: String,
    pub token: String,
    pub project: String,
    pub repository: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct NativeSession {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub base_url: String,
    pub runtime: String,
    pub native_id: String,
    pub turns: u32,
    pub model: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub proposal: Option<SupervisorProposal>,
    pub kind: String,
    pub node: String,
    pub questions: Vec<String>,
    pub resume_task: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AgentResult {
    pub status: String,
    pub summary: String,
    pub questions: Vec<String>,
    pub artifacts: Vec<String>,
    pub decision: String,
    pub details: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub details_truncated: bool,
}
impl AgentResult {
    pub fn report(&self) -> String {
        if self.details.is_empty() {
            self.summary.clone()
        } else {
            format!("{}\n\n{}", self.summary, self.details)
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub execution: ExecutionPolicy,
    #[serde(default)]
    pub parent: Option<ParentRun>,
    #[serde(default)]
    pub children: Vec<ChildRun>,
    pub id: String,
    pub workflow: Workflow,
    pub status: String,
    pub current: String,
    pub created: i64,
    pub updated: i64,
    pub workspace: String,
    pub states: BTreeMap<String, String>,
    pub sessions: BTreeMap<String, NativeSession>,
    pub outputs: BTreeMap<String, AgentResult>,
    pub pending: Option<Pending>,
    pub active_edge: Option<Edge>,
    pub pause_requested: bool,
    pub attempts: BTreeMap<String, u32>,
    pub reworks: BTreeMap<String, u32>,
    pub context: BTreeMap<String, String>,
    pub last_task: Option<String>,
    pub last_node: Option<String>,
    pub error: Option<String>,
    pub input: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentRun {
    pub run: String,
    pub node: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChildRun {
    pub run: String,
    pub node: String,
    pub attempt: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<ArtifactFile>,
    pub id: String,
    pub run: String,
    pub node: String,
    pub path: String,
    pub version: u32,
    pub hash: String,
    pub content: String,
    pub session: String,
    pub turn: u32,
    pub created: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactFile {
    #[serde(default)]
    pub modified: u64,
    pub root: String,
    pub location: String,
    pub change: String,
    pub size: u64,
    pub preview: bool,
    pub blob: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Decision {
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub supervisor: Option<SupervisorProposal>,
    pub id: String,
    pub run: String,
    pub node: String,
    pub decision: String,
    pub answer: String,
    pub user: String,
    pub created: i64,
    pub artifacts: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    pub run: String,
    pub kind: String,
    pub node: String,
    pub text: String,
    pub created: i64,
}
#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub name: String,
    pub transport: String,
    pub url: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub timeout_secs: u64,
    pub auth: McpAuth,
}
impl Default for McpConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            transport: "http".into(),
            url: String::new(),
            command: String::new(),
            args: vec![],
            env: BTreeMap::new(),
            headers: BTreeMap::new(),
            timeout_secs: 60,
            auth: McpAuth::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct McpAuth {
    pub mode: String,
    pub token: String,
    pub header: String,
    pub username: String,
    pub password: String,
    pub issuer: String,
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub token_auth: String,
    pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: i64,
}
impl Default for McpAuth {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            token: String::new(),
            header: "X-API-Key".into(),
            username: String::new(),
            password: String::new(),
            issuer: String::new(),
            authorization_url: String::new(),
            token_url: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            token_auth: "none".into(),
            scope: String::new(),
            access_token: String::new(),
            refresh_token: String::new(),
            expires: 0,
        }
    }
}
