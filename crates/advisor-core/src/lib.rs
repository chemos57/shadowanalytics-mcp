use market_context::{MarketContext, MarketDataHealth};
use serde::Serialize;
use thiserror::Error;

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

#[derive(Debug, Serialize)]
pub struct AdvisorSnapshot {
    pub question: String,
    pub liquidity_signals: LiquiditySignalBundle,
    pub market_context: MarketContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_context_health: Option<MarketDataHealth>,
    pub confirmations: Vec<AdvisorConfirmation>,
    pub regime: AdvisorRegime,
    pub unknowns: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AdvisorConfirmation {
    pub asset: String,
    pub macro_bias: String,
    pub market_trend: String,
    pub alignment: String,
    pub reason: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AdvisorRegime {
    pub macro_liquidity: String,
    pub market_risk: String,
    pub combined: String,
}

#[derive(Debug, Error)]
pub enum AdvisorSnapshotError {
    #[error(
        "market context health as_of {health_as_of} does not match market context as_of {market_context_as_of}"
    )]
    MarketContextHealthAsOfMismatch {
        market_context_as_of: String,
        health_as_of: String,
    },
}

pub fn build_advisor_snapshot(
    question: String,
    liquidity_signals: LiquiditySignalBundle,
    market_context: MarketContext,
) -> AdvisorSnapshot {
    build_advisor_snapshot_unchecked(question, liquidity_signals, market_context, None)
}

pub fn build_advisor_snapshot_with_health(
    question: String,
    liquidity_signals: LiquiditySignalBundle,
    market_context: MarketContext,
    market_context_health: Option<MarketDataHealth>,
) -> Result<AdvisorSnapshot, AdvisorSnapshotError> {
    validate_market_context_health(&market_context, market_context_health.as_ref())?;
    Ok(build_advisor_snapshot_unchecked(
        question,
        liquidity_signals,
        market_context,
        market_context_health,
    ))
}

fn build_advisor_snapshot_unchecked(
    question: String,
    liquidity_signals: LiquiditySignalBundle,
    market_context: MarketContext,
    market_context_health: Option<MarketDataHealth>,
) -> AdvisorSnapshot {
    let confirmations = confirmations(&liquidity_signals, &market_context);
    let regime = advisor_regime(&liquidity_signals, &market_context);
    let mut unknowns = liquidity_signals.unknowns.clone();
    push_unique(&mut unknowns, "No live data".to_string());
    push_unique(&mut unknowns, "No position sizing".to_string());
    push_unique(&mut unknowns, "No execution recommendation".to_string());
    push_unique(
        &mut unknowns,
        "Advisor snapshot is deterministic context, not financial advice".to_string(),
    );

    AdvisorSnapshot {
        question,
        liquidity_signals,
        market_context,
        market_context_health,
        confirmations,
        regime,
        unknowns,
    }
}

fn validate_market_context_health(
    market_context: &MarketContext,
    market_context_health: Option<&MarketDataHealth>,
) -> Result<(), AdvisorSnapshotError> {
    if let Some(health) = market_context_health {
        if health.as_of != market_context.as_of {
            return Err(AdvisorSnapshotError::MarketContextHealthAsOfMismatch {
                market_context_as_of: market_context.as_of.clone(),
                health_as_of: health.as_of.clone(),
            });
        }
    }
    Ok(())
}

fn confirmations(
    liquidity_signals: &LiquiditySignalBundle,
    market_context: &MarketContext,
) -> Vec<AdvisorConfirmation> {
    liquidity_signals
        .cross_asset_implications
        .iter()
        .map(|implication| confirmation(implication, market_context))
        .collect()
}

fn confirmation(
    implication: &CrossAssetImplication,
    market_context: &MarketContext,
) -> AdvisorConfirmation {
    let market_trend = market_context
        .assets
        .iter()
        .find(|asset| asset.symbol == implication.asset)
        .map(|asset| asset.trend_20d.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let alignment = alignment_for(&implication.bias, &market_trend);
    let reason = confirmation_reason(
        &implication.asset,
        &implication.bias,
        &market_trend,
        alignment,
    );

    AdvisorConfirmation {
        asset: implication.asset.clone(),
        macro_bias: implication.bias.clone(),
        market_trend,
        alignment: alignment.to_string(),
        reason,
    }
}

fn alignment_for(macro_bias: &str, market_trend: &str) -> &'static str {
    match expected_market_trend(macro_bias) {
        ExpectedTrend::Up => match market_trend {
            "up" => "aligned",
            "down" => "divergent",
            "flat" => "neutral",
            _ => "unknown",
        },
        ExpectedTrend::Down => match market_trend {
            "down" => "aligned",
            "up" => "divergent",
            "flat" => "neutral",
            _ => "unknown",
        },
        ExpectedTrend::Neutral => "neutral",
        ExpectedTrend::Unknown => "unknown",
    }
}

fn expected_market_trend(macro_bias: &str) -> ExpectedTrend {
    match macro_bias {
        "supportive" | "risk_supportive" | "defensive_supportive" => ExpectedTrend::Up,
        "less_supportive" | "risk_negative" => ExpectedTrend::Down,
        "ambiguous" => ExpectedTrend::Neutral,
        _ => ExpectedTrend::Unknown,
    }
}

fn confirmation_reason(
    asset: &str,
    macro_bias: &str,
    market_trend: &str,
    alignment: &str,
) -> String {
    match alignment {
        "aligned" => format!(
            "Macro liquidity bias is {macro_bias}, and {asset} trend is {market_trend}."
        ),
        "divergent" => format!(
            "Macro liquidity bias is {macro_bias}, but {asset} trend is {market_trend}."
        ),
        "neutral" => format!(
            "Macro liquidity bias is {macro_bias}, while {asset} trend is {market_trend}."
        ),
        _ => format!(
            "Macro liquidity bias is {macro_bias}, but {asset} market trend is unavailable or unsupported."
        ),
    }
}

fn advisor_regime(
    liquidity_signals: &LiquiditySignalBundle,
    market_context: &MarketContext,
) -> AdvisorRegime {
    let macro_liquidity = macro_liquidity_regime(liquidity_signals);
    let market_risk = market_context.cross_asset.risk_regime.clone();
    let combined = format!("macro_{macro_liquidity}_market_{market_risk}");

    AdvisorRegime {
        macro_liquidity,
        market_risk,
        combined,
    }
}

fn macro_liquidity_regime(liquidity_signals: &LiquiditySignalBundle) -> String {
    let tightening = liquidity_signals
        .liquidity_conditions
        .iter()
        .any(|condition| condition.direction == "tightening");
    let easing = liquidity_signals
        .liquidity_conditions
        .iter()
        .any(|condition| condition.direction == "easing");

    match (tightening, easing) {
        (true, true) => "mixed",
        (true, false) => "tightening",
        (false, true) => "easing",
        (false, false) => "unknown",
    }
    .to_string()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

enum ExpectedTrend {
    Up,
    Down,
    Neutral,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use market_context::{
        AssetContext, CrossAssetContext, MarketDataHealth, MarketDataHealthStatus,
    };

    #[test]
    fn builds_advisor_snapshot_with_alignment_and_regime() {
        let signals = LiquiditySignalBundle {
            question: "What does collateral scarcity imply?".to_string(),
            macro_themes: vec!["collateral".to_string(), "dollar_liquidity".to_string()],
            liquidity_conditions: vec![LiquidityCondition {
                label: "collateral_scarcity".to_string(),
                direction: "tightening".to_string(),
                confidence: "medium".to_string(),
                evidence: Vec::new(),
            }],
            cross_asset_implications: vec![
                CrossAssetImplication {
                    asset: "BTC".to_string(),
                    bias: "risk_negative".to_string(),
                    reason: "Macro liquidity pressure can weigh on risk assets.".to_string(),
                    citations: vec!["Signal.pdf:1".to_string()],
                },
                CrossAssetImplication {
                    asset: "DXY".to_string(),
                    bias: "supportive".to_string(),
                    reason: "Dollar funding stress can support dollar demand.".to_string(),
                    citations: vec!["Signal.pdf:1".to_string()],
                },
            ],
            unknowns: vec!["Corpus evidence only".to_string()],
            citations: vec!["Signal.pdf:1".to_string()],
        };
        let market_context = MarketContext {
            as_of: "2026-06-30".to_string(),
            assets: vec![
                asset("BTC", "up"),
                asset("DXY", "up"),
                asset("SPY", "up"),
                asset("QQQ", "up"),
            ],
            cross_asset: CrossAssetContext {
                risk_regime: "risk_on".to_string(),
                dxy_trend: "up".to_string(),
                rates_proxy: "TLT_up".to_string(),
            },
        };

        let snapshot = build_advisor_snapshot(
            "What does collateral scarcity imply?".to_string(),
            signals,
            market_context,
        );

        assert_eq!(snapshot.regime.macro_liquidity, "tightening");
        assert_eq!(snapshot.regime.market_risk, "risk_on");
        assert_eq!(snapshot.regime.combined, "macro_tightening_market_risk_on");
        assert!(snapshot.confirmations.iter().any(|confirmation| {
            confirmation.asset == "BTC"
                && confirmation.macro_bias == "risk_negative"
                && confirmation.market_trend == "up"
                && confirmation.alignment == "divergent"
        }));
        assert!(snapshot.confirmations.iter().any(|confirmation| {
            confirmation.asset == "DXY"
                && confirmation.macro_bias == "supportive"
                && confirmation.market_trend == "up"
                && confirmation.alignment == "aligned"
        }));
        assert!(snapshot
            .unknowns
            .contains(&"No execution recommendation".to_string()));
    }

    #[test]
    fn advisor_snapshot_can_include_market_context_health() {
        let signals = LiquiditySignalBundle {
            question: "What does collateral scarcity imply?".to_string(),
            macro_themes: vec!["collateral".to_string()],
            liquidity_conditions: Vec::new(),
            cross_asset_implications: Vec::new(),
            unknowns: Vec::new(),
            citations: Vec::new(),
        };
        let market_context = MarketContext {
            as_of: "2026-06-30".to_string(),
            assets: vec![asset("BTC", "up")],
            cross_asset: CrossAssetContext {
                risk_regime: "risk_on".to_string(),
                dxy_trend: "up".to_string(),
                rates_proxy: "TLT_up".to_string(),
            },
        };
        let health = MarketDataHealth {
            status: MarketDataHealthStatus::Ok,
            as_of: "2026-06-30".to_string(),
            missing_assets: Vec::new(),
            stale_assets: Vec::new(),
            warnings: Vec::new(),
            blocking_issues: Vec::new(),
        };

        let snapshot = build_advisor_snapshot_with_health(
            "What does collateral scarcity imply?".to_string(),
            signals,
            market_context,
            Some(health),
        )
        .unwrap();

        assert_eq!(
            snapshot.market_context_health.as_ref().unwrap().status,
            MarketDataHealthStatus::Ok
        );
    }

    #[test]
    fn advisor_snapshot_rejects_mismatched_market_context_health_as_of() {
        let signals = LiquiditySignalBundle {
            question: "What does collateral scarcity imply?".to_string(),
            macro_themes: vec!["collateral".to_string()],
            liquidity_conditions: Vec::new(),
            cross_asset_implications: Vec::new(),
            unknowns: Vec::new(),
            citations: Vec::new(),
        };
        let market_context = MarketContext {
            as_of: "2026-06-30".to_string(),
            assets: vec![asset("BTC", "up")],
            cross_asset: CrossAssetContext {
                risk_regime: "risk_on".to_string(),
                dxy_trend: "up".to_string(),
                rates_proxy: "TLT_up".to_string(),
            },
        };
        let health = MarketDataHealth {
            status: MarketDataHealthStatus::Ok,
            as_of: "2026-06-29".to_string(),
            missing_assets: Vec::new(),
            stale_assets: Vec::new(),
            warnings: Vec::new(),
            blocking_issues: Vec::new(),
        };

        let error = build_advisor_snapshot_with_health(
            "What does collateral scarcity imply?".to_string(),
            signals,
            market_context,
            Some(health),
        )
        .unwrap_err();

        assert!(error.to_string().contains(
            "market context health as_of 2026-06-29 does not match market context as_of 2026-06-30"
        ));
    }

    fn asset(symbol: &str, trend_20d: &str) -> AssetContext {
        AssetContext {
            symbol: symbol.to_string(),
            last_close: 100.0,
            return_1d: Some(0.01),
            return_5d: Some(0.02),
            return_20d: Some(0.05),
            trend_20d: trend_20d.to_string(),
            volatility_20d: Some(0.2),
            drawdown_20d: Some(0.0),
        }
    }
}
