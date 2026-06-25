use crate::chunk::KnowledgeChunk;
use qdrant_client::qdrant::Value;
use std::collections::HashMap;

pub fn chunk_payload(chunk: &KnowledgeChunk) -> HashMap<String, Value> {
    HashMap::from([
        ("doc_id".to_string(), Value::from(chunk.doc_id.clone())),
        (
            "file_name".to_string(),
            Value::from(chunk.file_name.clone()),
        ),
        ("page".to_string(), Value::from(chunk.page as i64)),
        (
            "chunk_index".to_string(),
            Value::from(chunk.chunk_index as i64),
        ),
        ("title".to_string(), Value::from(chunk.title.clone())),
        ("text".to_string(), Value::from(chunk.text.clone())),
        ("themes".to_string(), Value::from(chunk.themes.clone())),
        ("citation".to_string(), Value::from(chunk.citation.clone())),
    ])
}
