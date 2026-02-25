use std::time::Duration;

use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::info;

const REFRESH_INTERVAL: Duration = Duration::from_secs(300); // 5 minutes

/// Symbols to display in the market ticker panel.
const SYMBOLS: &[(&str, &str)] = &[
    ("^GSPC", "S&P 500"),
    ("^DJI", "DOW"),
    ("^IXIC", "NASDAQ"),
    ("^VIX", "VIX"),
    ("^TNX", "10Y"),
];

#[derive(Debug, Clone)]
pub struct MarketQuote {
    pub name: String,
    pub price: f64,
    pub change_pct: f64,
}

/// Spawn a background task that fetches market quotes on startup
/// and every REFRESH_INTERVAL thereafter.
pub fn spawn_market_feed(tx: mpsc::UnboundedSender<Vec<MarketQuote>>) {
    tokio::spawn(async move {
        loop {
            let quotes = fetch_all_quotes().await;
            if tx.send(quotes).is_err() {
                break; // receiver dropped
            }
            tokio::time::sleep(REFRESH_INTERVAL).await;
        }
    });
}

async fn fetch_all_quotes() -> Vec<MarketQuote> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let mut quotes = Vec::new();

    // Fetch each symbol via the v8 chart endpoint (still public)
    for &(symbol, name) in SYMBOLS {
        match fetch_one(&client, symbol).await {
            Some((price, change_pct)) => {
                quotes.push(MarketQuote {
                    name: name.to_string(),
                    price,
                    change_pct,
                });
            }
            None => {
                quotes.push(MarketQuote {
                    name: name.to_string(),
                    price: 0.0,
                    change_pct: 0.0,
                });
            }
        }
    }

    info!(count = quotes.len(), "market: quotes loaded");
    quotes
}

async fn fetch_one(client: &reqwest::Client, symbol: &str) -> Option<(f64, f64)> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=2d",
        symbol
    );

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?;

    let body: ChartResponse = response.json().await.ok()?;
    let result = body.chart.result?.into_iter().next()?;

    let price = result.meta.regular_market_price?;
    let prev_close = result.meta.chart_previous_close?;

    let change_pct = if prev_close > 0.0 {
        ((price - prev_close) / prev_close) * 100.0
    } else {
        0.0
    };

    Some((price, change_pct))
}

#[derive(Deserialize)]
struct ChartResponse {
    chart: ChartBody,
}

#[derive(Deserialize)]
struct ChartBody {
    result: Option<Vec<ChartResult>>,
}

#[derive(Deserialize)]
struct ChartResult {
    meta: ChartMeta,
}

#[derive(Deserialize)]
struct ChartMeta {
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: Option<f64>,
    #[serde(rename = "chartPreviousClose")]
    chart_previous_close: Option<f64>,
}
