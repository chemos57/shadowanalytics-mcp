use crate::chunk::KnowledgeChunk;

const THEME_RULES: &[(&str, &[&str])] = &[
    (
        "collateral",
        &["collateral", "safe asset", "safe assets", "treasury"],
    ),
    ("repo", &["repo", "reverse repo", "rrp", "sofr"]),
    (
        "dollar_liquidity",
        &["dollar liquidity", "reserves", "eurodollar", "funding"],
    ),
    (
        "fx_swaps",
        &["fx swap", "fx swaps", "cross-currency", "basis"],
    ),
    (
        "shadow_banking",
        &["shadow banking", "dealer", "balance sheet"],
    ),
    (
        "commodities",
        &["commodity", "commodities", "bretton woods iii"],
    ),
    (
        "crypto_liquidity",
        &["bitcoin", "btc", "stablecoin", "crypto"],
    ),
];

pub fn tag_chunk(mut chunk: KnowledgeChunk) -> KnowledgeChunk {
    let lower = chunk.text.to_ascii_lowercase();
    chunk.themes = THEME_RULES
        .iter()
        .filter_map(|(theme, needles)| {
            needles
                .iter()
                .any(|needle| lower.contains(needle))
                .then(|| theme.to_string())
        })
        .collect();
    chunk
}

pub fn tag_chunks(chunks: Vec<KnowledgeChunk>) -> Vec<KnowledgeChunk> {
    chunks.into_iter().map(tag_chunk).collect()
}
