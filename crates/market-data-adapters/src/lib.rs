use anyhow::{bail, Context, Result};
use chrono::{Duration, NaiveDate, Utc};
use market_context::{build_market_context, MarketContext, PriceRow};
pub use market_context::{MarketDataHealth, MarketDataHealthStatus};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchMarketContextRequest {
    pub assets: Vec<String>,
    pub lookback_days: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FetchMarketContextResult {
    pub context: MarketContext,
    pub health: MarketDataHealth,
}

pub trait MarketDataProvider {
    fn fetch_prices(&self, request: &FetchMarketContextRequest) -> Result<Vec<PriceRow>>;
}

pub struct YahooCsvProvider {
    client: reqwest::blocking::Client,
}

impl YahooCsvProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for YahooCsvProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MarketDataProvider for YahooCsvProvider {
    fn fetch_prices(&self, request: &FetchMarketContextRequest) -> Result<Vec<PriceRow>> {
        let end = Utc::now().date_naive();
        let start = end - Duration::days(i64::from(request.lookback_days));
        let mut rows = Vec::new();

        for asset in normalized_assets(&request.assets) {
            let url = yahoo_download_url(&asset, start, end)?;
            let csv = self
                .client
                .get(&url)
                .send()
                .with_context(|| format!("fetch {asset} prices from Yahoo-compatible CSV"))?
                .error_for_status()
                .with_context(|| format!("Yahoo-compatible CSV request failed for {asset}"))?
                .text()
                .with_context(|| format!("read Yahoo-compatible CSV response for {asset}"))?;
            rows.extend(parse_yahoo_csv(&asset, &csv)?);
        }

        Ok(rows)
    }
}

pub fn build_market_context_from_provider<P: MarketDataProvider>(
    provider: &P,
    request: &FetchMarketContextRequest,
    today: NaiveDate,
    max_stale_days: i64,
) -> Result<FetchMarketContextResult> {
    let rows = provider.fetch_prices(request)?;
    let context = build_market_context(&rows)?;
    let health = health_for_market_data(
        &rows,
        &context,
        &normalized_assets(&request.assets),
        today,
        max_stale_days,
    );

    Ok(FetchMarketContextResult { context, health })
}

pub fn build_market_context_from_yahoo(
    request: &FetchMarketContextRequest,
    max_stale_days: i64,
) -> Result<FetchMarketContextResult> {
    build_market_context_from_provider(
        &YahooCsvProvider::new(),
        request,
        Utc::now().date_naive(),
        max_stale_days,
    )
}

pub fn parse_yahoo_csv(asset: &str, csv: &str) -> Result<Vec<PriceRow>> {
    let mut lines = csv.lines().enumerate();
    let Some((_, header)) = lines.next() else {
        bail!("Yahoo-compatible CSV is empty for {asset}");
    };
    let columns = split_csv_line(header);
    let date_index = column_index(&columns, "Date")?;
    let close_index = column_index(&columns, "Close")?;
    let mut rows = Vec::new();

    for (line_index, line) in lines {
        if line.trim().is_empty() {
            continue;
        }
        let values = split_csv_line(line);
        let line_number = line_index + 1;
        let date = values
            .get(date_index)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && value != "null")
            .with_context(|| format!("missing date for {asset} at line {line_number}"))?;
        let close = values
            .get(close_index)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty() && *value != "null")
            .with_context(|| format!("missing close for {asset} at line {line_number}"))?
            .parse::<f64>()
            .with_context(|| format!("invalid close for {asset} at line {line_number}"))?;
        if !close.is_finite() {
            bail!("close must be finite for {asset} at line {line_number}");
        }
        if close <= 0.0 {
            bail!("close must be positive for {asset} at line {line_number}");
        }
        rows.push(PriceRow {
            date,
            symbol: asset.trim().to_ascii_uppercase(),
            close,
        });
    }

    if rows.is_empty() {
        bail!("Yahoo-compatible CSV has no price rows for {asset}");
    }

    Ok(rows)
}

pub fn health_for_market_data(
    rows: &[PriceRow],
    context: &MarketContext,
    required_assets: &[String],
    today: NaiveDate,
    max_stale_days: i64,
) -> MarketDataHealth {
    let required = normalized_assets(required_assets);
    let present = context
        .assets
        .iter()
        .map(|asset| asset.symbol.clone())
        .collect::<BTreeSet<_>>();
    let latest_dates = latest_dates_by_symbol(rows);

    let missing_assets = required
        .iter()
        .filter(|asset| !present.contains(*asset))
        .cloned()
        .collect::<Vec<_>>();
    let stale_assets = stale_assets(&required, &latest_dates, today, max_stale_days);
    let mut warnings = Vec::new();
    let mut blocking_issues = Vec::new();

    if !missing_assets.is_empty() {
        blocking_issues.push(format!("missing assets: {}", missing_assets.join(",")));
    }
    if !stale_assets.is_empty() {
        blocking_issues.push(format!("stale assets: {}", stale_assets.join(",")));
    }
    for asset in &context.assets {
        if asset.return_20d.is_none()
            || asset.volatility_20d.is_none()
            || asset.trend_20d == "unknown"
        {
            warnings.push(format!("insufficient history for {}", asset.symbol));
        }
    }
    if context.cross_asset.risk_regime == "mixed" {
        warnings.push("cross-asset risk regime is mixed".to_string());
    }

    warnings.sort();
    warnings.dedup();
    let status = if !blocking_issues.is_empty() {
        MarketDataHealthStatus::Invalid
    } else if !warnings.is_empty() {
        MarketDataHealthStatus::Warning
    } else {
        MarketDataHealthStatus::Ok
    };

    MarketDataHealth {
        status,
        as_of: context.as_of.clone(),
        missing_assets,
        stale_assets,
        warnings,
        blocking_issues,
    }
}

pub fn yahoo_symbol(asset: &str) -> &'static str {
    match asset.trim().to_ascii_uppercase().as_str() {
        "BTC" => "BTC-USD",
        "ETH" => "ETH-USD",
        "DXY" => "DX-Y.NYB",
        "SPY" => "SPY",
        "QQQ" => "QQQ",
        "GLD" => "GLD",
        "TLT" => "TLT",
        _ => "",
    }
}

pub fn supported_assets() -> Vec<&'static str> {
    vec!["BTC", "ETH", "SPY", "QQQ", "GLD", "TLT", "DXY"]
}

fn yahoo_download_url(asset: &str, start: NaiveDate, end: NaiveDate) -> Result<String> {
    let symbol = yahoo_symbol(asset);
    if symbol.is_empty() {
        bail!("unsupported Yahoo-compatible asset symbol: {asset}");
    }
    let period1 = start
        .and_hms_opt(0, 0, 0)
        .context("invalid Yahoo start date")?
        .and_utc()
        .timestamp();
    let period2 = (end + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .context("invalid Yahoo end date")?
        .and_utc()
        .timestamp();

    Ok(format!(
        "https://query1.finance.yahoo.com/v7/finance/download/{symbol}?period1={period1}&period2={period2}&interval=1d&events=history&includeAdjustedClose=true"
    ))
}

fn normalized_assets(assets: &[String]) -> Vec<String> {
    let mut values = assets
        .iter()
        .map(|asset| asset.trim().to_ascii_uppercase())
        .filter(|asset| !asset.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort_by_key(|asset| {
        supported_assets()
            .iter()
            .position(|supported| supported == asset)
            .unwrap_or(usize::MAX)
    });
    values
}

fn latest_dates_by_symbol(rows: &[PriceRow]) -> BTreeMap<String, NaiveDate> {
    let mut latest = BTreeMap::<String, NaiveDate>::new();
    for row in rows {
        let Ok(date) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") else {
            continue;
        };
        latest
            .entry(row.symbol.trim().to_ascii_uppercase())
            .and_modify(|current| {
                if date > *current {
                    *current = date;
                }
            })
            .or_insert(date);
    }
    latest
}

fn stale_assets(
    required_assets: &[String],
    latest_dates: &BTreeMap<String, NaiveDate>,
    today: NaiveDate,
    max_stale_days: i64,
) -> Vec<String> {
    required_assets
        .iter()
        .filter(|asset| {
            latest_dates
                .get(*asset)
                .map(|latest| today.signed_duration_since(*latest).num_days() > max_stale_days)
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn column_index(columns: &[String], name: &str) -> Result<usize> {
    columns
        .iter()
        .position(|column| column.trim().eq_ignore_ascii_case(name))
        .with_context(|| format!("missing required Yahoo-compatible CSV column: {name}"))
}

fn split_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|value| value.trim().trim_matches('"').to_string())
        .collect()
}
