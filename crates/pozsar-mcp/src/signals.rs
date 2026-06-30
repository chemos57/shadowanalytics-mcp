use crate::research::{answer_research_question, ResearchEvidence, ResearchQuestionParams};
use pozsar_kb::chunk::KnowledgeChunk;
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LiquiditySignalParams {
    pub question: String,
    pub assets: Vec<String>,
    pub themes: Option<Vec<String>>,
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct LiquiditySignalBundle {
    pub question: String,
    pub macro_themes: Vec<String>,
    pub liquidity_conditions: Vec<LiquidityCondition>,
    pub cross_asset_implications: Vec<CrossAssetImplication>,
    pub unknowns: Vec<String>,
    pub citations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LiquidityCondition {
    pub label: String,
    pub direction: String,
    pub confidence: String,
    pub evidence: Vec<LiquiditySignalEvidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiquiditySignalEvidence {
    pub citation: String,
    pub doc_id: String,
    pub page: u32,
    pub themes: Vec<String>,
    pub snippet: Option<String>,
    pub query_sources: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CrossAssetImplication {
    pub asset: String,
    pub bias: String,
    pub reason: String,
    pub citations: Vec<String>,
}

pub fn extract_liquidity_signals(
    chunks: &[KnowledgeChunk],
    params: LiquiditySignalParams,
) -> LiquiditySignalBundle {
    let research = answer_research_question(
        chunks,
        ResearchQuestionParams {
            question: params.question.clone(),
            themes: params.themes.clone(),
            doc_id: None,
            limit: params.limit,
        },
    );

    let macro_themes = params
        .themes
        .filter(|themes| !themes.is_empty())
        .unwrap_or_else(|| themes_from_evidence(&research.evidence));
    let liquidity_conditions = liquidity_conditions_from_evidence(&research.evidence);
    let citations = research.citations.clone();
    let cross_asset_implications = cross_asset_implications_from_conditions(
        &normalize_assets(&params.assets),
        &liquidity_conditions,
    );

    LiquiditySignalBundle {
        question: research.question,
        macro_themes,
        liquidity_conditions,
        cross_asset_implications,
        unknowns: vec![
            "No live market data included".to_string(),
            "Corpus evidence only".to_string(),
            "No execution recommendation, position sizing, or risk limit included".to_string(),
            "A trading advisor must combine this with market data, volatility, trend, positioning, and risk rules".to_string(),
        ],
        citations,
    }
}

fn liquidity_conditions_from_evidence(evidence: &[ResearchEvidence]) -> Vec<LiquidityCondition> {
    let specs = [
        ConditionSpec {
            label: "collateral_scarcity",
            direction: "tightening",
            primary_terms: &["collateral"],
            signal_terms: &[
                "scarcity",
                "scarce",
                "shortage",
                "shortages",
                "tighten",
                "tightens",
                "tightened",
                "tightening",
                "tighter",
                "stress",
                "stressed",
                "stresses",
            ],
        },
        ConditionSpec {
            label: "dollar_liquidity_tightness",
            direction: "tightening",
            primary_terms: &["dollar_liquidity", "dollar liquidity"],
            signal_terms: &[
                "tighten",
                "tightens",
                "tightened",
                "tightening",
                "tighter",
                "stress",
                "stressed",
                "stresses",
                "scarcity",
                "scarce",
                "shortage",
                "shortages",
            ],
        },
        ConditionSpec {
            label: "repo_stress",
            direction: "tightening",
            primary_terms: &["repo"],
            signal_terms: &[
                "stress",
                "stressed",
                "stresses",
                "tighten",
                "tightens",
                "tightened",
                "tightening",
                "tighter",
                "scarcity",
                "scarce",
                "shortage",
                "shortages",
            ],
        },
        ConditionSpec {
            label: "fx_swap_stress",
            direction: "tightening",
            primary_terms: &["fx_swaps", "fx swap", "swap"],
            signal_terms: &[
                "stress",
                "stressed",
                "stresses",
                "demand",
                "demands",
                "tighten",
                "tightens",
                "tightened",
                "tightening",
                "tighter",
                "shortage",
                "shortages",
            ],
        },
        ConditionSpec {
            label: "safe_asset_demand",
            direction: "tightening",
            primary_terms: &["safe asset", "safe assets", "treasury", "treasuries"],
            signal_terms: &[
                "demand",
                "demands",
                "scarcity",
                "scarce",
                "shortage",
                "shortages",
                "stress",
                "stressed",
                "stresses",
                "liquidity",
            ],
        },
        ConditionSpec {
            label: "dollar_liquidity_easing",
            direction: "easing",
            primary_terms: &["dollar_liquidity", "dollar liquidity", "funding"],
            signal_terms: &[
                "ease",
                "eased",
                "easing",
                "easier",
                "ample",
                "abundant",
                "abundance",
                "looser",
                "improve",
                "improved",
                "improves",
                "improving",
            ],
        },
        ConditionSpec {
            label: "collateral_abundance",
            direction: "easing",
            primary_terms: &["collateral"],
            signal_terms: &[
                "ample",
                "abundant",
                "abundance",
                "easier",
                "looser",
                "improve",
                "improved",
                "improves",
                "improving",
            ],
        },
    ];

    specs
        .iter()
        .filter_map(|spec| condition_from_spec(spec, evidence))
        .collect()
}

fn condition_from_spec(
    spec: &ConditionSpec<'_>,
    evidence: &[ResearchEvidence],
) -> Option<LiquidityCondition> {
    let matches = evidence
        .iter()
        .filter(|item| evidence_matches_condition(item, spec))
        .take(4)
        .map(signal_evidence)
        .collect::<Vec<_>>();

    (!matches.is_empty()).then(|| LiquidityCondition {
        label: spec.label.to_string(),
        direction: spec.direction.to_string(),
        confidence: confidence_for_evidence(matches.len()).to_string(),
        evidence: matches,
    })
}

fn evidence_matches_condition(item: &ResearchEvidence, spec: &ConditionSpec<'_>) -> bool {
    let searchable = evidence_searchable_text(item);
    let primary_match = spec
        .primary_terms
        .iter()
        .any(|term| term_matches(&searchable, term));
    let signal_match = spec
        .signal_terms
        .iter()
        .any(|term| term_matches(&searchable, term));

    primary_match && signal_match
}

fn term_matches(searchable: &str, term: &str) -> bool {
    if term.contains(' ') {
        return searchable.contains(term);
    }

    searchable
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|token| token == term)
}

fn evidence_searchable_text(item: &ResearchEvidence) -> String {
    format!(
        "{} {} {}",
        item.passage.text,
        item.snippet.as_deref().unwrap_or_default(),
        item.passage.themes.join(" ")
    )
    .to_ascii_lowercase()
}

fn signal_evidence(item: &ResearchEvidence) -> LiquiditySignalEvidence {
    LiquiditySignalEvidence {
        citation: item.citation.clone(),
        doc_id: item.passage.doc_id.clone(),
        page: item.passage.page,
        themes: item.passage.themes.clone(),
        snippet: item.snippet.clone().or_else(|| {
            let text = item
                .passage
                .text
                .split_whitespace()
                .take(40)
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then_some(text)
        }),
        query_sources: item.query_sources.clone(),
    }
}

fn confidence_for_evidence(count: usize) -> &'static str {
    match count {
        0 => "low",
        1 => "medium",
        _ => "high",
    }
}

fn cross_asset_implications_from_conditions(
    assets: &[String],
    conditions: &[LiquidityCondition],
) -> Vec<CrossAssetImplication> {
    let regime = liquidity_regime(conditions);
    let citations = citations_from_conditions(conditions);

    assets
        .iter()
        .map(|asset| implication_for_asset(asset, regime, &citations))
        .collect()
}

fn citations_from_conditions(conditions: &[LiquidityCondition]) -> Vec<String> {
    let mut citations = Vec::new();

    for condition in conditions {
        for evidence in &condition.evidence {
            if !citations.contains(&evidence.citation) {
                citations.push(evidence.citation.clone());
            }
            if citations.len() >= 5 {
                return citations;
            }
        }
    }

    citations
}

fn implication_for_asset(
    asset: &str,
    regime: LiquidityRegime,
    citations: &[String],
) -> CrossAssetImplication {
    let (bias, reason) = match (asset, regime) {
        ("DXY", LiquidityRegime::Tightening) => (
            "supportive",
            "Corpus evidence points to tighter dollar funding conditions, which can increase demand for dollar liquidity.",
        ),
        ("DXY", LiquidityRegime::Easing) => (
            "less_supportive",
            "Corpus evidence points to easier dollar liquidity conditions, which can reduce pressure for defensive dollar demand.",
        ),
        ("BTC" | "ETH" | "SPY" | "QQQ", LiquidityRegime::Tightening) => (
            "risk_negative",
            "Corpus evidence points to liquidity tightening, a macro condition that can pressure duration-sensitive or speculative risk assets.",
        ),
        ("BTC" | "ETH" | "SPY" | "QQQ", LiquidityRegime::Easing) => (
            "risk_supportive",
            "Corpus evidence points to easier liquidity, a macro condition that can support risk appetite when market confirmation is present.",
        ),
        ("GLD", LiquidityRegime::Tightening) => (
            "defensive_supportive",
            "Corpus evidence points to liquidity stress, where defensive and monetary assets may become more relevant, but market confirmation is required.",
        ),
        ("TLT", LiquidityRegime::Tightening) => (
            "ambiguous",
            "Corpus evidence points to liquidity stress; safe-asset demand can support Treasuries while funding stress can also raise volatility.",
        ),
        (_, LiquidityRegime::Mixed) => (
            "ambiguous",
            "Corpus evidence is mixed or insufficient for a deterministic cross-asset liquidity bias.",
        ),
        (_, LiquidityRegime::Unknown) => (
            "unknown",
            "Corpus evidence is insufficient for a deterministic cross-asset liquidity bias.",
        ),
        _ => (
            "unknown",
            "No deterministic corpus-only mapping is configured for this asset and liquidity regime.",
        ),
    };

    CrossAssetImplication {
        asset: asset.to_string(),
        bias: bias.to_string(),
        reason: reason.to_string(),
        citations: citations.to_vec(),
    }
}

fn liquidity_regime(conditions: &[LiquidityCondition]) -> LiquidityRegime {
    let tightening = conditions
        .iter()
        .any(|condition| condition.direction == "tightening");
    let easing = conditions
        .iter()
        .any(|condition| condition.direction == "easing");

    match (tightening, easing) {
        (true, true) => LiquidityRegime::Mixed,
        (true, false) => LiquidityRegime::Tightening,
        (false, true) => LiquidityRegime::Easing,
        (false, false) => LiquidityRegime::Unknown,
    }
}

fn normalize_assets(assets: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();

    for asset in assets {
        let symbol = asset.trim().to_ascii_uppercase();
        if !symbol.is_empty() && seen.insert(symbol.clone()) {
            normalized.push(symbol);
        }
    }

    normalized
}

fn themes_from_evidence(evidence: &[ResearchEvidence]) -> Vec<String> {
    evidence
        .iter()
        .flat_map(|item| item.passage.themes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct ConditionSpec<'a> {
    label: &'a str,
    direction: &'a str,
    primary_terms: &'a [&'a str],
    signal_terms: &'a [&'a str],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiquidityRegime {
    Tightening,
    Easing,
    Mixed,
    Unknown,
}
