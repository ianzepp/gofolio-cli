mod accounts;
mod activities;
mod assets;
mod benchmarks;
mod performance;
mod portfolio;

use crate::api::{ApiError, GhostfolioClient};

pub async fn dispatch(
    client: &GhostfolioClient,
    tool_name: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    match tool_name {
        "get_portfolio_summary" => portfolio::get_portfolio_summary(client).await,
        "get_holdings" => portfolio::get_holdings(client).await,
        "get_holding_detail" => portfolio::get_holding_detail(client, input).await,
        "get_performance" => performance::get_performance(client, input).await,
        "get_dividends" => performance::get_dividends(client, input).await,
        "get_investments" => performance::get_investments(client, input).await,
        "list_activities" => activities::list_activities(client).await,
        "list_accounts" => accounts::list_accounts(client).await,
        "get_account_balances" => accounts::get_account_balances(client, input).await,
        "search_assets" => assets::search_assets(client, input).await,
        "get_asset_profile" => assets::get_asset_profile(client, input).await,
        "get_market_data" => assets::get_market_data(client).await,
        "get_benchmarks" => benchmarks::get_benchmarks(client).await,
        _ => Err(ApiError::Request(format!("unknown tool: {tool_name}"))),
    }
}
