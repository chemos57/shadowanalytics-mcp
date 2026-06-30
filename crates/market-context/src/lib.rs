use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct PriceRow {
    pub date: String,
    pub symbol: String,
    pub close: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketContext {
    pub as_of: String,
    pub assets: Vec<AssetContext>,
    pub cross_asset: CrossAssetContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetContext {
    pub symbol: String,
    pub last_close: f64,
    pub return_1d: Option<f64>,
    pub return_5d: Option<f64>,
    pub return_20d: Option<f64>,
    pub trend_20d: String,
    pub volatility_20d: Option<f64>,
    pub drawdown_20d: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossAssetContext {
    pub risk_regime: String,
    pub dxy_trend: String,
    pub rates_proxy: String,
}

#[derive(Debug, Clone)]
struct PricePoint {
    date: String,
    close: f64,
}

pub fn build_market_context_from_csv(path: &Path) -> Result<MarketContext> {
    let csv = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let rows = parse_price_csv(&csv)?;
    build_market_context(&rows)
}

pub fn parse_price_csv(csv: &str) -> Result<Vec<PriceRow>> {
    let mut lines = csv.lines().enumerate();
    let Some((_, header)) = lines.next() else {
        bail!("price CSV is empty");
    };
    let columns = parse_csv_line(header);
    let date_index = column_index(&columns, "date")?;
    let symbol_index = column_index(&columns, "symbol")?;
    let close_index = column_index(&columns, "close")?;
    let mut rows = Vec::new();

    for (line_index, line) in lines {
        if line.trim().is_empty() {
            continue;
        }
        let values = parse_csv_line(line);
        let line_number = line_index + 1;
        let date = values
            .get(date_index)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .with_context(|| format!("missing date at line {line_number}"))?;
        let symbol = values
            .get(symbol_index)
            .map(|value| value.trim().to_ascii_uppercase())
            .filter(|value| !value.is_empty())
            .with_context(|| format!("missing symbol at line {line_number}"))?;
        let close = values
            .get(close_index)
            .with_context(|| format!("missing close at line {line_number}"))?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("invalid close at line {line_number}"))?;
        if !close.is_finite() {
            bail!("close must be finite at line {line_number}");
        }
        if close <= 0.0 {
            bail!("close must be positive at line {line_number}");
        }
        rows.push(PriceRow {
            date,
            symbol,
            close,
        });
    }

    if rows.is_empty() {
        bail!("price CSV has no data rows");
    }

    Ok(rows)
}

pub fn build_market_context(rows: &[PriceRow]) -> Result<MarketContext> {
    if rows.is_empty() {
        bail!("market context requires at least one price row");
    }

    let as_of = rows
        .iter()
        .map(|row| row.date.as_str())
        .max()
        .unwrap_or_default()
        .to_string();
    let mut grouped = BTreeMap::<String, Vec<PricePoint>>::new();
    for row in rows {
        grouped
            .entry(row.symbol.trim().to_ascii_uppercase())
            .or_default()
            .push(PricePoint {
                date: row.date.clone(),
                close: row.close,
            });
    }

    let mut assets = Vec::new();
    for (symbol, mut points) in grouped {
        points.sort_by(|left, right| left.date.cmp(&right.date));
        dedupe_same_day_points(&mut points);
        if let Some(asset) = asset_context(&symbol, &points) {
            assets.push(asset);
        }
    }

    let cross_asset = cross_asset_context(&assets);

    Ok(MarketContext {
        as_of,
        assets,
        cross_asset,
    })
}

fn asset_context(symbol: &str, points: &[PricePoint]) -> Option<AssetContext> {
    let last = points.last()?;
    let return_1d = trailing_return(points, 1);
    let return_5d = trailing_return(points, 5);
    let return_20d = trailing_return(points, 20);
    let trend_20d = trend_from_return(return_20d).to_string();
    let volatility_20d = volatility(points, 20);
    let drawdown_20d = drawdown(points, 20);

    Some(AssetContext {
        symbol: symbol.to_string(),
        last_close: round6(last.close),
        return_1d,
        return_5d,
        return_20d,
        trend_20d,
        volatility_20d,
        drawdown_20d,
    })
}

fn trailing_return(points: &[PricePoint], lookback: usize) -> Option<f64> {
    if points.len() <= lookback {
        return None;
    }
    let last = points.last()?.close;
    let previous = points.get(points.len() - 1 - lookback)?.close;
    Some(round6(last / previous - 1.0))
}

fn volatility(points: &[PricePoint], lookback: usize) -> Option<f64> {
    if points.len() <= lookback {
        return None;
    }
    let start = points.len() - 1 - lookback;
    let returns = points[start..]
        .windows(2)
        .map(|window| window[1].close / window[0].close - 1.0)
        .collect::<Vec<_>>();
    if returns.is_empty() {
        return None;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f64>()
        / returns.len() as f64;
    Some(round6(variance.sqrt() * 252.0_f64.sqrt()))
}

fn drawdown(points: &[PricePoint], lookback: usize) -> Option<f64> {
    let last = points.last()?.close;
    let start = points.len().saturating_sub(lookback + 1);
    let peak = points[start..]
        .iter()
        .map(|point| point.close)
        .fold(f64::NEG_INFINITY, f64::max);
    if !peak.is_finite() || peak <= 0.0 {
        return None;
    }
    Some(round6(last / peak - 1.0))
}

fn trend_from_return(value: Option<f64>) -> &'static str {
    match value {
        Some(value) if value > 0.005 => "up",
        Some(value) if value < -0.005 => "down",
        Some(_) => "flat",
        None => "unknown",
    }
}

fn cross_asset_context(assets: &[AssetContext]) -> CrossAssetContext {
    let dxy_trend = asset_trend(assets, "DXY").unwrap_or("unknown").to_string();
    let tlt_trend = asset_trend(assets, "TLT").unwrap_or("unknown");
    let rates_proxy = match tlt_trend {
        "up" => "TLT_up",
        "down" => "TLT_down",
        "flat" => "TLT_flat",
        _ => "unknown",
    }
    .to_string();
    let risk_regime = risk_regime(assets, &dxy_trend);

    CrossAssetContext {
        risk_regime,
        dxy_trend,
        rates_proxy,
    }
}

fn risk_regime(assets: &[AssetContext], dxy_trend: &str) -> String {
    let mut score = 0;
    for symbol in ["BTC", "ETH", "SPY", "QQQ"] {
        match asset_trend(assets, symbol) {
            Some("up") => score += 1,
            Some("down") => score -= 1,
            _ => {}
        }
    }
    match dxy_trend {
        "down" => score += 1,
        "up" => score -= 1,
        _ => {}
    }

    match score {
        score if score >= 2 => "risk_on",
        score if score <= -2 => "risk_off",
        _ => "mixed",
    }
    .to_string()
}

fn asset_trend<'a>(assets: &'a [AssetContext], symbol: &str) -> Option<&'a str> {
    assets
        .iter()
        .find(|asset| asset.symbol == symbol)
        .map(|asset| asset.trend_20d.as_str())
}

fn dedupe_same_day_points(points: &mut Vec<PricePoint>) {
    let mut deduped = Vec::<PricePoint>::new();
    for point in points.drain(..) {
        if let Some(last) = deduped.last_mut() {
            if last.date == point.date {
                *last = point;
                continue;
            }
        }
        deduped.push(point);
    }
    *points = deduped;
}

fn column_index(columns: &[String], name: &str) -> Result<usize> {
    columns
        .iter()
        .position(|column| column.trim().eq_ignore_ascii_case(name))
        .with_context(|| format!("missing required CSV column: {name}"))
}

fn parse_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|value| value.trim().to_string())
        .collect()
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_market_context_with_returns_and_cross_asset_regime() {
        let mut rows = Vec::new();
        for day in 1..=21 {
            let date = format!("2026-06-{day:02}");
            rows.push(row(&date, "BTC", 100.0 + day as f64));
            rows.push(row(&date, "SPY", 500.0 + day as f64));
            rows.push(row(&date, "QQQ", 400.0 + day as f64));
            rows.push(row(&date, "DXY", 120.0 - day as f64));
            rows.push(row(&date, "TLT", 90.0 + day as f64));
        }

        let context = build_market_context(&rows).unwrap();
        let btc = context
            .assets
            .iter()
            .find(|asset| asset.symbol == "BTC")
            .unwrap();

        assert_eq!(context.as_of, "2026-06-21");
        assert_eq!(btc.last_close, 121.0);
        assert_eq!(btc.return_1d, Some(0.008333));
        assert_eq!(btc.return_5d, Some(0.043103));
        assert_eq!(btc.return_20d, Some(0.19802));
        assert_eq!(btc.trend_20d, "up");
        assert!(btc.volatility_20d.unwrap() > 0.0);
        assert_eq!(btc.drawdown_20d, Some(0.0));
        assert_eq!(context.cross_asset.risk_regime, "risk_on");
        assert_eq!(context.cross_asset.dxy_trend, "down");
        assert_eq!(context.cross_asset.rates_proxy, "TLT_up");
    }

    #[test]
    fn parses_price_csv_and_normalizes_symbols() {
        let rows =
            parse_price_csv("date,symbol,close\n2026-06-24,btc,101000\n2026-06-25,BTC,103000\n")
                .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "BTC");
        assert_eq!(rows[0].close, 101000.0);
    }

    #[test]
    fn rejects_invalid_price_rows() {
        let error = parse_price_csv("date,symbol,close\n2026-06-24,BTC,0\n")
            .unwrap_err()
            .to_string();

        assert!(error.contains("close must be positive"));
    }

    #[test]
    fn rejects_non_finite_price_rows() {
        for close in ["NaN", "inf", "-inf"] {
            let error = parse_price_csv(&format!("date,symbol,close\n2026-06-24,BTC,{close}\n"))
                .unwrap_err()
                .to_string();

            assert!(
                error.contains("close must be finite"),
                "unexpected error for {close}: {error}"
            );
        }
    }

    fn row(date: &str, symbol: &str, close: f64) -> PriceRow {
        PriceRow {
            date: date.to_string(),
            symbol: symbol.to_string(),
            close,
        }
    }
}
