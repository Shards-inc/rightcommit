use super::model::*;
use anyhow::{Result, ensure};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::Path;
pub fn model(id: &str) -> Result<EmbeddingModel> {
    Ok(match id {
        "multilingual-minilm" => EmbeddingModel::ParaphraseMLMiniLML12V2,
        "bge-small-en" => EmbeddingModel::BGESmallENV15,
        "jina-code" => EmbeddingModel::JinaEmbeddingsV2BaseCode,
        _ => anyhow::bail!("Modello embedding non valido"),
    })
}
pub fn load(cfg: &Config, cache: &Path) -> Result<TextEmbedding> {
    TextEmbedding::try_new(
        InitOptions::new(model(&cfg.model)?)
            .with_cache_dir(cache.into())
            .with_max_length(cfg.encoder_tokens)
            .with_show_download_progress(false),
    )
}
// Swiftide determines structural chunks. Only chunks exceeding the encoder window
// are subdivided, preserving exact UTF-8 source offsets and all source content.
pub fn fit(
    engine: &TextEmbedding,
    chunks: Vec<Chunk>,
    cfg: &Config,
) -> Result<(Vec<Chunk>, usize)> {
    let mut tokenizer = engine.tokenizer.clone();
    tokenizer
        .with_truncation(None)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut out = vec![];
    let mut splits = 0;
    for c in chunks {
        let enc = tokenizer
            .encode(c.text.as_str(), false)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let budget = cfg.encoder_tokens.saturating_sub(8);
        if enc.len() <= budget {
            out.push(c);
            continue;
        }
        let mut start = 0;
        let offsets = enc.get_offsets();
        let mut token = 0;
        while start < c.text.len() {
            let mut end_token = (token + budget).min(offsets.len());
            let mut end = if end_token == offsets.len() {
                c.text.len()
            } else {
                offsets[end_token].0
            };
            while end_token < offsets.len() && (end <= start || !c.text.is_char_boundary(end)) {
                end_token += 1;
                end = if end_token == offsets.len() {
                    c.text.len()
                } else {
                    offsets[end_token].0
                }
            }
            ensure!(end > start, "Finestra encoder non valida");
            let mut child = c.clone();
            child.text = c.text[start..end].into();
            child.byte_start = c.byte_start + start;
            child.byte_end = c.byte_start + end;
            child.line_start =
                c.line_start + c.text[..start].bytes().filter(|b| *b == b'\n').count();
            child.line_end = child.line_start + child.text.bytes().filter(|b| *b == b'\n').count()
                - usize::from(child.text.ends_with('\n'));
            child.id = crate::store::hash(format!(
                "{}:{}:{}:{}",
                c.path, c.source_hash, child.byte_start, child.byte_end
            ));
            child.metadata["encoder_subdivision"] = true.into();
            out.push(child);
            start = end;
            token = end_token;
        }
        splits += 1;
    }
    Ok((out, splits))
}
pub fn query(engine: &TextEmbedding, text: String, cfg: &Config) -> Result<Vec<f32>> {
    let mut tokenizer = engine.tokenizer.clone();
    tokenizer
        .with_truncation(None)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let tokens = tokenizer
        .encode(text.as_str(), true)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .len();
    ensure!(
        tokens <= cfg.encoder_tokens,
        "La query supera il limite encoder di {} token; usa una domanda più breve",
        cfg.encoder_tokens
    );
    Ok(engine.embed(vec![text], None)?.remove(0))
}
