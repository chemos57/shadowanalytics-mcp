use advisor_core::AdvisorSnapshot;
use market_context::{MarketDataHealth, MarketDataHealthStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisorPolicy {
    pub as_of: String,
    pub regime: String,
    pub asset_assessments: Vec<AssetAssessment>,
    pub unknowns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAssessment {
    pub asset: String,
    pub macro_bias: String,
    pub market_trend: String,
    pub alignment: String,
    pub stance: String,
    pub confidence: String,
    pub drivers: Vec<String>,
    pub risks: Vec<String>,
    pub required_checks: Vec<String>,
}

pub fn build_advisor_policy(snapshot: AdvisorSnapshot) -> AdvisorPolicy {
    let asset_assessments = snapshot
        .confirmations
        .iter()
        .map(|confirmation| {
            let implication = snapshot
                .liquidity_signals
                .cross_asset_implications
                .iter()
                .find(|implication| implication.asset == confirmation.asset);
            let evidence_count = implication
                .map(|implication| implication.citations.len())
                .unwrap_or_default();

            AssetAssessment {
                asset: confirmation.asset.clone(),
                macro_bias: confirmation.macro_bias.clone(),
                market_trend: confirmation.market_trend.clone(),
                alignment: confirmation.alignment.clone(),
                stance: stance_for(&confirmation.macro_bias, &confirmation.alignment).to_string(),
                confidence: confidence_for(
                    &confirmation.alignment,
                    evidence_count,
                    snapshot.market_context_health.as_ref(),
                )
                .to_string(),
                drivers: drivers_for(
                    &confirmation.asset,
                    &confirmation.macro_bias,
                    &confirmation.market_trend,
                ),
                risks: risks_for(
                    &confirmation.alignment,
                    snapshot.market_context_health.as_ref(),
                ),
                required_checks: required_checks(),
            }
        })
        .collect();

    let mut unknowns = snapshot.unknowns;
    push_unique(&mut unknowns, "No positions".to_string());
    push_unique(&mut unknowns, "No account risk limits".to_string());
    push_unique(&mut unknowns, "No execution recommendation".to_string());

    AdvisorPolicy {
        as_of: snapshot.market_context.as_of,
        regime: snapshot.regime.combined,
        asset_assessments,
        unknowns,
    }
}

fn stance_for(macro_bias: &str, alignment: &str) -> &'static str {
    match alignment {
        "aligned" => match macro_bias {
            "supportive" | "risk_supportive" | "defensive_supportive" => "favorable",
            "risk_negative" | "less_supportive" => "unfavorable",
            "ambiguous" => "neutral",
            _ => "unknown",
        },
        "divergent" => "watch",
        "neutral" => "neutral",
        _ => "unknown",
    }
}

fn confidence_for(
    alignment: &str,
    evidence_count: usize,
    health: Option<&MarketDataHealth>,
) -> &'static str {
    if matches!(
        health.map(|health| &health.status),
        Some(MarketDataHealthStatus::Invalid)
    ) {
        return "low";
    }
    if alignment == "unknown" || evidence_count == 0 {
        return "low";
    }
    if matches!(
        health.map(|health| &health.status),
        Some(MarketDataHealthStatus::Warning)
    ) {
        return "medium";
    }
    if alignment == "aligned" && evidence_count >= 2 {
        "high"
    } else {
        "medium"
    }
}

fn drivers_for(asset: &str, macro_bias: &str, market_trend: &str) -> Vec<String> {
    vec![
        format!(
            "Macro liquidity evidence is {}",
            macro_bias.replace('_', "-")
        ),
        trend_driver(asset, market_trend),
    ]
}

fn trend_driver(asset: &str, market_trend: &str) -> String {
    match market_trend {
        "up" => format!("{asset} trend is still up"),
        "down" => format!("{asset} trend is down"),
        "flat" => format!("{asset} trend is flat"),
        _ => format!("{asset} trend is unavailable"),
    }
}

fn risks_for(alignment: &str, health: Option<&MarketDataHealth>) -> Vec<String> {
    let mut risks = Vec::new();
    match alignment {
        "divergent" => push_unique(&mut risks, "Macro/market divergence".to_string()),
        "unknown" => push_unique(
            &mut risks,
            "Market trend or macro bias is unknown".to_string(),
        ),
        _ => {}
    }
    if let Some(health) = health {
        match health.status {
            MarketDataHealthStatus::Invalid => {
                push_unique(&mut risks, "Market context health is invalid".to_string())
            }
            MarketDataHealthStatus::Warning => {
                push_unique(&mut risks, "Market context health has warnings".to_string())
            }
            MarketDataHealthStatus::Ok => {}
        }
        if !health.missing_assets.is_empty() {
            push_unique(&mut risks, "Market context has missing assets".to_string());
        }
        if !health.stale_assets.is_empty() {
            push_unique(&mut risks, "Market context has stale assets".to_string());
        }
    } else {
        push_unique(&mut risks, "No market context health metadata".to_string());
    }
    push_unique(&mut risks, "No positioning data".to_string());
    push_unique(&mut risks, "No volatility regime rules".to_string());
    risks
}

fn required_checks() -> Vec<String> {
    vec![
        "Confirm current volatility".to_string(),
        "Check portfolio exposure".to_string(),
        "Check invalidation level".to_string(),
    ]
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use advisor_core::{
        AdvisorConfirmation, AdvisorRegime, AdvisorSnapshot, CrossAssetImplication,
        LiquiditySignalBundle,
    };
    use market_context::{
        CrossAssetContext, MarketContext, MarketDataHealth, MarketDataHealthStatus,
    };

    #[test]
    fn policy_marks_macro_market_divergence_as_watch() {
        let policy = build_advisor_policy(snapshot(
            "2026-06-30",
            Some(MarketDataHealthStatus::Ok),
            vec![AdvisorConfirmation {
                asset: "BTC".to_string(),
                macro_bias: "risk_negative".to_string(),
                market_trend: "up".to_string(),
                alignment: "divergent".to_string(),
                reason: "Macro liquidity bias is risk_negative, but BTC trend is up.".to_string(),
            }],
            vec![CrossAssetImplication {
                asset: "BTC".to_string(),
                bias: "risk_negative".to_string(),
                reason: "Liquidity tightening can pressure speculative assets.".to_string(),
                citations: vec!["Signal.pdf:1".to_string(), "Signal.pdf:2".to_string()],
            }],
        ));

        assert_eq!(policy.as_of, "2026-06-30");
        assert_eq!(policy.regime, "macro_tightening_market_risk_on");
        assert_eq!(policy.asset_assessments.len(), 1);
        let assessment = &policy.asset_assessments[0];
        assert_eq!(assessment.asset, "BTC");
        assert_eq!(assessment.macro_bias, "risk_negative");
        assert_eq!(assessment.market_trend, "up");
        assert_eq!(assessment.alignment, "divergent");
        assert_eq!(assessment.stance, "watch");
        assert_eq!(assessment.confidence, "medium");
        assert_eq!(
            assessment.drivers,
            vec![
                "Macro liquidity evidence is risk-negative",
                "BTC trend is still up",
            ]
        );
        assert!(assessment
            .risks
            .contains(&"Macro/market divergence".to_string()));
        assert!(assessment
            .risks
            .contains(&"No positioning data".to_string()));
        assert!(assessment
            .risks
            .contains(&"No volatility regime rules".to_string()));
        assert_eq!(
            assessment.required_checks,
            vec![
                "Confirm current volatility",
                "Check portfolio exposure",
                "Check invalidation level",
            ]
        );
        assert!(policy.unknowns.contains(&"No positions".to_string()));
        assert!(policy
            .unknowns
            .contains(&"No account risk limits".to_string()));
        assert!(policy
            .unknowns
            .contains(&"No execution recommendation".to_string()));
    }

    #[test]
    fn policy_degrades_confidence_when_market_health_is_invalid() {
        let policy = build_advisor_policy(snapshot(
            "2026-06-30",
            Some(MarketDataHealthStatus::Invalid),
            vec![AdvisorConfirmation {
                asset: "DXY".to_string(),
                macro_bias: "supportive".to_string(),
                market_trend: "up".to_string(),
                alignment: "aligned".to_string(),
                reason: "Macro liquidity bias is supportive, and DXY trend is up.".to_string(),
            }],
            vec![CrossAssetImplication {
                asset: "DXY".to_string(),
                bias: "supportive".to_string(),
                reason: "Dollar funding stress can support dollar demand.".to_string(),
                citations: vec!["Signal.pdf:1".to_string(), "Signal.pdf:2".to_string()],
            }],
        ));

        let assessment = &policy.asset_assessments[0];
        assert_eq!(assessment.stance, "favorable");
        assert_eq!(assessment.confidence, "low");
        assert!(assessment
            .risks
            .contains(&"Market context health is invalid".to_string()));
    }

    fn snapshot(
        as_of: &str,
        health_status: Option<MarketDataHealthStatus>,
        confirmations: Vec<AdvisorConfirmation>,
        implications: Vec<CrossAssetImplication>,
    ) -> AdvisorSnapshot {
        AdvisorSnapshot {
            question: "What does collateral scarcity imply?".to_string(),
            liquidity_signals: LiquiditySignalBundle {
                question: "What does collateral scarcity imply?".to_string(),
                macro_themes: vec!["collateral".to_string(), "dollar_liquidity".to_string()],
                liquidity_conditions: Vec::new(),
                cross_asset_implications: implications,
                unknowns: vec!["Corpus evidence only".to_string()],
                citations: vec!["Signal.pdf:1".to_string()],
            },
            market_context: MarketContext {
                as_of: as_of.to_string(),
                assets: Vec::new(),
                cross_asset: CrossAssetContext {
                    risk_regime: "risk_on".to_string(),
                    dxy_trend: "up".to_string(),
                    rates_proxy: "TLT_up".to_string(),
                },
            },
            market_context_health: health_status.map(|status| MarketDataHealth {
                status,
                as_of: as_of.to_string(),
                missing_assets: Vec::new(),
                stale_assets: Vec::new(),
                warnings: Vec::new(),
                blocking_issues: Vec::new(),
            }),
            confirmations,
            regime: AdvisorRegime {
                macro_liquidity: "tightening".to_string(),
                market_risk: "risk_on".to_string(),
                combined: "macro_tightening_market_risk_on".to_string(),
            },
            unknowns: vec!["No execution recommendation".to_string()],
        }
    }
}
