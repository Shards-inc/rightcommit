use crate::model::now;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub strategy: String,
    pub language: String,
    pub min_size: usize,
    pub max_size: usize,
    pub outline: bool,
    pub outline_min_bytes: usize,
    pub symbols: bool,
    pub model: String,
    pub encoder_tokens: usize,
    pub batch_size: usize,
    pub retrieval: String,
    pub top_k: usize,
    pub recursive: bool,
    pub hidden: bool,
    pub extensions: Vec<String>,
    pub max_file_mb: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            strategy: "auto".into(),
            language: "auto".into(),
            min_size: 0,
            max_size: 1500,
            outline: true,
            outline_min_bytes: 0,
            symbols: true,
            model: "multilingual-minilm".into(),
            encoder_tokens: 512,
            batch_size: 8,
            retrieval: "hybrid".into(),
            top_k: 8,
            recursive: true,
            hidden: false,
            extensions: [
                "md", "txt", "rst", "rs", "py", "js", "jsx", "ts", "tsx", "java", "go", "rb", "cs",
                "sol", "c", "h", "cpp", "hpp", "ex", "exs", "html", "php", "json", "yaml", "yml",
                "toml", "sql", "sh", "css",
            ]
            .map(String::from)
            .to_vec(),
            max_file_mb: 2,
        }
    }
}
impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::ensure;
        ensure!(
            ["auto", "code", "markdown", "text"].contains(&self.strategy.as_str()),
            "Strategia chunking non valida"
        );
        ensure!(
            ["semantic", "keyword", "hybrid"].contains(&self.retrieval.as_str()),
            "Modalità ricerca non valida"
        );
        ensure!(
            self.min_size < self.max_size && (128..=8192).contains(&self.max_size),
            "Dimensione chunk: 128–8192, minimo inferiore al massimo"
        );
        ensure!(
            (64..=512).contains(&self.encoder_tokens)
                && (1..=32).contains(&self.batch_size)
                && (1..=30).contains(&self.top_k),
            "Parametri encoder, batch o risultati non validi"
        );
        ensure!(
            (1..=10).contains(&self.max_file_mb)
                && self.extensions.len() <= 80
                && self.extensions.iter().all(|s| !s.is_empty()
                    && s.len() < 16
                    && s.chars().all(|c| c.is_ascii_alphanumeric())),
            "Estensioni o limite file non validi"
        );
        crate::rag::embedding::model(&self.model)?;
        if self.language != "auto" {
            crate::rag::chunking::language(&self.language)?;
        }
        Ok(())
    }
    pub fn content_key(&self) -> String {
        let mut c = self.clone();
        c.top_k = 0;
        c.batch_size = 0;
        c.retrieval = if self.retrieval == "keyword" {
            "keyword"
        } else {
            "vector"
        }
        .into();
        crate::store::hash(serde_json::to_vec(&c).unwrap())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Stats {
    pub files: usize,
    pub chunks: usize,
    pub new_files: usize,
    pub modified_files: usize,
    pub deleted_files: usize,
    pub reused_files: usize,
    pub skipped_files: usize,
    pub embedded_chunks: usize,
    pub reused_chunks: usize,
    pub encoder_splits: usize,
    pub scan_ms: u64,
    pub chunk_ms: u64,
    pub embedding_ms: u64,
    pub storage_ms: u64,
    pub total_ms: u64,
    pub size_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub state: String,
    pub phase: String,
    pub done: usize,
    pub total: Option<usize>,
    pub message: String,
    pub files_done: usize,
    pub files_total: usize,
    pub chunks: usize,
    pub started: i64,
    pub updated: i64,
}
impl Default for Progress {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            phase: "idle".into(),
            done: 0,
            total: None,
            message: String::new(),
            files_done: 0,
            files_total: 0,
            chunks: 0,
            started: now(),
            updated: now(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Index {
    pub id: String,
    pub root: String,
    pub path: String,
    pub name: String,
    pub directory: bool,
    pub config: Config,
    pub created: i64,
    pub updated: i64,
    pub generation: Option<String>,
    pub generation_config: Option<Config>,
    pub progress: Progress,
    pub stats: Stats,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stamp {
    pub hash: String,
    pub size: u64,
    pub modified: u128,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub files: BTreeMap<String, Stamp>,
    pub content_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub path: String,
    pub source_hash: String,
    pub text: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub language: String,
    pub metadata: serde_json::Value,
    #[serde(skip)]
    pub vector: Vec<f32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Search {
    pub query: String,
    pub mode: String,
    pub top_k: usize,
    pub path_prefix: String,
    pub extension: String,
}
impl Default for Search {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: String::new(),
            top_k: 8,
            path_prefix: String::new(),
            extension: String::new(),
        }
    }
}
