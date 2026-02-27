use tokio::sync::oneshot;
use tracing::warn;

use crate::api::GhostfolioClient;

/// Structured portfolio data for the sidebar display.
#[derive(Debug, Clone, Default)]
pub struct PortfolioSummary {
    pub total_value: Option<f64>,
    pub total_investment: Option<f64>,
    pub net_performance: Option<f64>,
    pub net_performance_pct: Option<f64>,
    pub currency: String,
    pub num_holdings: usize,
    pub num_accounts: usize,
    pub top_accounts: Vec<AccountRow>,
    pub top_holdings: Vec<HoldingRow>,
}

#[derive(Debug, Clone)]
pub struct HoldingRow {
    pub name: String,
    pub allocation_pct: f64,
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub name: String,
    pub value: f64,
}

pub fn spawn_warmup(client: GhostfolioClient) -> oneshot::Receiver<PortfolioSummary> {
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let summary = fetch_portfolio_summary(&client).await;
        let _ = tx.send(summary);
    });
    rx
}

async fn fetch_portfolio_summary(client: &GhostfolioClient) -> PortfolioSummary {
    let (accounts, holdings, performance) = tokio::join!(
        client.get("/api/v1/account"),
        client.get("/api/v1/portfolio/holdings"),
        client.get_with_query("/api/v2/portfolio/performance", &[("range", "max")]),
    );

    let mut summary = PortfolioSummary::default();

    match accounts {
        Ok(data) => extract_accounts(&data, &mut summary),
        Err(e) => warn!(error = %e, "warmup: accounts fetch failed"),
    }
    match holdings {
        Ok(data) => extract_holdings(&data, &mut summary),
        Err(e) => warn!(error = %e, "warmup: holdings fetch failed"),
    }
    match performance {
        Ok(data) => extract_performance(&data, &mut summary),
        Err(e) => warn!(error = %e, "warmup: performance fetch failed"),
    }

    summary
}

fn extract_holdings(data: &serde_json::Value, summary: &mut PortfolioSummary) {
    let arr = data
        .get("holdings")
        .and_then(|v| v.as_array())
        .or_else(|| data.as_array());
    let Some(holdings) = arr else { return };

    summary.num_holdings = holdings.len();
    let mut rows: Vec<HoldingRow> = holdings.iter().filter_map(parse_holding_row).collect();
    rows.sort_by(|a, b| {
        b.allocation_pct
            .partial_cmp(&a.allocation_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(5);
    summary.top_holdings = rows;
}

fn extract_accounts(data: &serde_json::Value, summary: &mut PortfolioSummary) {
    let arr = data
        .get("accounts")
        .and_then(|v| v.as_array())
        .or_else(|| data.as_array());
    let Some(accounts) = arr else { return };

    summary.num_accounts = accounts.len();
    if summary.total_value.is_none() {
        summary.total_value = data
            .get("totalValueInBaseCurrency")
            .and_then(|v| v.as_f64())
            .or_else(|| data.get("totalValue").and_then(|v| v.as_f64()));
    }

    let mut rows: Vec<AccountRow> = accounts.iter().filter_map(parse_account_row).collect();
    rows.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(5);
    summary.top_accounts = rows;
}

fn extract_performance(data: &serde_json::Value, summary: &mut PortfolioSummary) {
    let perf = data.get("performance").unwrap_or(data);

    summary.total_value = summary.total_value.or_else(|| {
        perf.get("currentValueInBaseCurrency")
            .and_then(|v| v.as_f64())
            .or_else(|| perf.get("currentValue").and_then(|v| v.as_f64()))
            .or_else(|| perf.get("currentNetWorth").and_then(|v| v.as_f64()))
    });
    summary.total_investment = perf.get("totalInvestment").and_then(|v| v.as_f64());
    summary.net_performance = perf
        .get("netPerformanceWithCurrencyEffect")
        .and_then(|v| v.as_f64())
        .or_else(|| perf.get("netPerformance").and_then(|v| v.as_f64()));
    summary.net_performance_pct = perf
        .get("netPerformancePercentageWithCurrencyEffect")
        .and_then(|v| v.as_f64())
        .or_else(|| {
            perf.get("netPerformancePercentage")
                .and_then(|v| v.as_f64())
        })
        .or_else(|| perf.get("netPerformancePercent").and_then(|v| v.as_f64()));

    if let Some(currency) = perf.get("currency").and_then(|v| v.as_str()) {
        summary.currency = currency.to_string();
    }
}

fn parse_account_row(a: &serde_json::Value) -> Option<AccountRow> {
    let name = a.get("name").and_then(|v| v.as_str())?;
    let value = a
        .get("valueInBaseCurrency")
        .and_then(|v| v.as_f64())
        .or_else(|| a.get("balanceInBaseCurrency").and_then(|v| v.as_f64()))
        .or_else(|| a.get("balance").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    Some(AccountRow {
        name: name.to_string(),
        value,
    })
}

fn parse_holding_row(h: &serde_json::Value) -> Option<HoldingRow> {
    let name = h
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| h.get("symbol").and_then(|v| v.as_str()))?;
    let alloc = h
        .get("allocationInPercentage")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    Some(HoldingRow {
        name: name.to_string(),
        allocation_pct: alloc * 100.0,
    })
}
