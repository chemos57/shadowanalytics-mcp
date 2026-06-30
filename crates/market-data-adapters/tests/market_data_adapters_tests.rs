use chrono::NaiveDate;
use market_context::PriceRow;
use market_data_adapters::{
    build_market_context_from_provider, health_for_market_data, parse_yahoo_csv, yahoo_symbol,
    FetchMarketContextRequest, MarketDataHealthStatus, MarketDataProvider,
};

struct FixtureProvider {
    rows: Vec<PriceRow>,
}

impl MarketDataProvider for FixtureProvider {
    fn fetch_prices(&self, _request: &FetchMarketContextRequest) -> anyhow::Result<Vec<PriceRow>> {
        Ok(self.rows.clone())
    }
}

#[test]
fn parses_yahoo_csv_into_normalized_price_rows() {
    let csv = "Date,Open,High,Low,Close,Adj Close,Volume\n\
2026-06-26,100,101,99,100.50,100.50,1000\n\
2026-06-27,101,102,100,101.25,101.25,1200\n";

    let rows = parse_yahoo_csv("BTC", csv).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].date, "2026-06-26");
    assert_eq!(rows[0].symbol, "BTC");
    assert_eq!(rows[0].close, 100.50);
    assert_eq!(rows[1].close, 101.25);
}

#[test]
fn maps_cross_asset_symbols_to_yahoo_symbols() {
    assert_eq!(yahoo_symbol("BTC"), "BTC-USD");
    assert_eq!(yahoo_symbol("ETH"), "ETH-USD");
    assert_eq!(yahoo_symbol("SPY"), "SPY");
    assert_eq!(yahoo_symbol("QQQ"), "QQQ");
    assert_eq!(yahoo_symbol("GLD"), "GLD");
    assert_eq!(yahoo_symbol("TLT"), "TLT");
    assert_eq!(yahoo_symbol("DXY"), "DX-Y.NYB");
}

#[test]
fn builds_market_context_from_provider_rows() {
    let rows = fixture_rows(&["BTC", "ETH", "SPY", "QQQ", "GLD", "TLT", "DXY"], 21);
    let provider = FixtureProvider { rows };
    let request = FetchMarketContextRequest {
        assets: vec!["BTC".to_string(), "DXY".to_string()],
        lookback_days: 60,
    };

    let result = build_market_context_from_provider(
        &provider,
        &request,
        NaiveDate::from_ymd_opt(2026, 6, 25).unwrap(),
        7,
    )
    .unwrap();

    assert_eq!(result.context.as_of, "2026-06-21");
    assert!(result
        .context
        .assets
        .iter()
        .any(|asset| asset.symbol == "BTC" && asset.trend_20d == "up"));
    assert_eq!(result.health.status, MarketDataHealthStatus::Ok);
    assert!(result.health.blocking_issues.is_empty());
}

#[test]
fn health_flags_missing_and_stale_assets() {
    let rows = fixture_rows(&["BTC"], 21);
    let context = market_context::build_market_context(&rows).unwrap();
    let health = health_for_market_data(
        &rows,
        &context,
        &["BTC".to_string(), "DXY".to_string()],
        NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
        3,
    );

    assert_eq!(health.status, MarketDataHealthStatus::Invalid);
    assert_eq!(health.missing_assets, vec!["DXY"]);
    assert_eq!(health.stale_assets, vec!["BTC"]);
    assert!(health
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("missing assets")));
    assert!(health
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("stale assets")));
}

fn fixture_rows(symbols: &[&str], days: u32) -> Vec<PriceRow> {
    let mut rows = Vec::new();
    for day in 1..=days {
        let date = format!("2026-06-{day:02}");
        for symbol in symbols {
            let close = if *symbol == "DXY" {
                120.0 - day as f64
            } else {
                100.0 + day as f64
            };
            rows.push(PriceRow {
                date: date.clone(),
                symbol: (*symbol).to_string(),
                close,
            });
        }
    }
    rows
}
