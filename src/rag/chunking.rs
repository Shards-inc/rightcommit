use super::model::*;
use anyhow::{Context, Result, ensure};
use futures::TryStreamExt;
use serde_json::json;
use std::path::Path;
use swiftide::integrations::treesitter::{
    CodeOutliner, CodeParser, CodeSplitter, SupportedLanguages,
};
use swiftide::{
    indexing::{
        TextNode,
        transformers::{ChunkMarkdown, ChunkText},
    },
    traits::ChunkerTransformer,
};
pub fn language(s: &str) -> Result<SupportedLanguages> {
    s.try_into()
        .map_err(|_| anyhow::anyhow!("Linguaggio non supportato"))
}
pub fn languages() -> Vec<serde_json::Value> {
    ["rust","python","javascript","typescript","ruby","java","go","csharp","solidity","c","cpp","elixir","html","php"].into_iter().map(|s|{let l=language(s).unwrap();json!({"id":s,"name":l.to_string(),"extensions":l.file_extensions(),"outline":outline_supported(l),"symbols":symbols_supported(l)})}).collect()
}
fn outline_supported(l: SupportedLanguages) -> bool {
    matches!(
        l,
        SupportedLanguages::Rust
            | SupportedLanguages::Python
            | SupportedLanguages::Javascript
            | SupportedLanguages::Typescript
            | SupportedLanguages::Java
            | SupportedLanguages::Ruby
            | SupportedLanguages::CSharp
    )
}
fn symbols_supported(l: SupportedLanguages) -> bool {
    outline_supported(l) || matches!(l, SupportedLanguages::Go | SupportedLanguages::Solidity)
}
fn detect(path: &str) -> Option<SupportedLanguages> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();
    languages()
        .into_iter()
        .find(|l| {
            l["extensions"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&ext)))
        })
        .and_then(|l| language(l["id"].as_str()?).ok())
}
pub fn provenance(
    path: &str,
    source: &str,
    hash: &str,
    start: usize,
    end: usize,
    lang: &str,
    metadata: serde_json::Value,
) -> Chunk {
    Chunk {
        id: crate::store::hash(format!("{path}:{hash}:{start}:{end}")),
        path: path.into(),
        source_hash: hash.into(),
        text: source[start..end].into(),
        byte_start: start,
        byte_end: end,
        line_start: source[..start].bytes().filter(|c| *c == b'\n').count() + 1,
        line_end: source[..end].bytes().filter(|c| *c == b'\n').count()
            + usize::from(end == 0 || !source[..end].ends_with('\n')),
        language: lang.into(),
        metadata,
        vector: vec![],
    }
}
pub async fn chunks(
    path: &str,
    text: &str,
    hash: &str,
    cfg: &Config,
) -> Result<(Vec<Chunk>, Vec<String>)> {
    let detected = if cfg.language == "auto" {
        detect(path)
    } else {
        Some(language(&cfg.language)?)
    };
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let strategy = if cfg.strategy == "auto" {
        if detected.is_some() {
            "code"
        } else if ["md", "markdown"].contains(&ext) {
            "markdown"
        } else {
            "text"
        }
    } else {
        &cfg.strategy
    };
    let mut metadata = json!({"strategy":strategy,"engine":"Swiftide 0.32.1"});
    let mut warnings = vec![];
    let parts = match strategy {
        "code" => {
            let l = detected.context(
                "Seleziona un linguaggio per questo file: rilevamento automatico non disponibile",
            )?;
            if cfg.outline && text.len() >= cfg.outline_min_bytes {
                if outline_supported(l) {
                    let outline = CodeOutliner::new(l).outline(text)?;
                    if outline.len() <= 32768 {
                        metadata["outline"] = json!(outline)
                    } else {
                        warnings.push(format!("{path}: outline oltre 32 KB omesso"));
                    }
                } else {
                    warnings.push(format!(
                        "{path}: outline non disponibile per {l}"
                    ))
                }
            }
            let splitter = CodeSplitter::builder()
                .try_language(l)?
                .chunk_size(cfg.min_size..cfg.max_size)
                .build()?;
            splitter.split(text)?
        }
        "markdown" => ChunkMarkdown::from_chunk_range(cfg.min_size..cfg.max_size)
            .transform_node(TextNode::new(text.to_string()))
            .await
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .map(|n| n.chunk)
            .collect(),
        _ => ChunkText::from_chunk_range(cfg.min_size..cfg.max_size)
            .transform_node(TextNode::new(text.to_string()))
            .await
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .map(|n| n.chunk)
            .collect(),
    };
    let mut cursor = 0;
    let mut out = vec![];
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let start = cursor
            + text[cursor..]
                .find(&part)
                .context("Frammento non riconducibile al sorgente")?;
        let end = start + part.len();
        ensure!(
            text.is_char_boundary(start) && text.is_char_boundary(end),
            "Offset UTF-8 non valido"
        );
        cursor = end;
        let mut meta = metadata.clone();
        if !out.is_empty() {
            meta.as_object_mut().unwrap().remove("outline");
        }
        if cfg.symbols && strategy == "code" {
            let l = detected.unwrap();
            if symbols_supported(l) {
                let parsed = CodeParser::from_language(l)
                    .parse(&part)?
                    .references_and_definitions()?;
                let mut refs = parsed.references.into_iter().collect::<Vec<_>>();
                refs.sort();
                let mut defs = parsed.definitions.into_iter().collect::<Vec<_>>();
                defs.sort();
                meta["definitions"] = json!(defs);
                meta["references"] = json!(refs);
            } else if warnings.iter().all(|w| !w.contains("riferimenti")) {
                warnings.push(format!(
                    "{path}: definizioni e riferimenti non disponibili per {l}"
                ))
            }
        }
        out.push(provenance(
            path,
            text,
            hash,
            start,
            end,
            &detected.map(|l| l.to_string()).unwrap_or_default(),
            meta,
        ));
    }
    Ok((out, warnings))
}
