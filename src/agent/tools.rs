use super::types::Tool;

pub fn all_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_portfolio_summary".to_string(),
            description: "Get a detailed summary of the user's portfolio including holdings, \
                allocations, performance metrics, and current market values."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_holdings".to_string(),
            description: "Get all current portfolio holdings with their symbols, names, \
                quantities, market values, and allocation percentages."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_holding_detail".to_string(),
            description: "Get detailed information about a specific holding including \
                performance history, transactions, and current metrics."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "dataSource": {
                        "type": "string",
                        "description": "Data source identifier (e.g., 'YAHOO')"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Ticker symbol (e.g., 'AAPL')"
                    }
                },
                "required": ["dataSource", "symbol"]
            }),
        },
        Tool {
            name: "get_performance".to_string(),
            description: "Get portfolio performance data over a time range including \
                returns, gains/losses, and comparison benchmarks."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range: '1d', '1w', '1m', '3m', '6m', 'ytd', '1y', '3y', '5y', 'max'",
                        "enum": ["1d", "1w", "1m", "3m", "6m", "ytd", "1y", "3y", "5y", "max"]
                    }
                },
                "required": ["range"]
            }),
        },
        Tool {
            name: "get_dividends".to_string(),
            description: "Get dividend income data for the portfolio.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range: '1d', '1w', '1m', '3m', '6m', 'ytd', '1y', '3y', '5y', 'max'",
                        "enum": ["1d", "1w", "1m", "3m", "6m", "ytd", "1y", "3y", "5y", "max"]
                    }
                },
                "required": ["range"]
            }),
        },
        Tool {
            name: "get_investments".to_string(),
            description: "Get investment contribution data over time.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range: '1d', '1w', '1m', '3m', '6m', 'ytd', '1y', '3y', '5y', 'max'",
                        "enum": ["1d", "1w", "1m", "3m", "6m", "ytd", "1y", "3y", "5y", "max"]
                    }
                },
                "required": ["range"]
            }),
        },
        Tool {
            name: "list_activities".to_string(),
            description: "List all portfolio activities (trades, dividends, fees, etc.) \
                with dates, types, symbols, quantities, and amounts."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "list_accounts".to_string(),
            description: "List all investment accounts with their names, types, \
                balances, and platform information."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_account_balances".to_string(),
            description: "Get balance history for a specific account.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Account UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "search_assets".to_string(),
            description: "Search for financial assets by name or ticker symbol. \
                Returns matching symbols with their names, data sources, and asset types."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search term (company name or ticker symbol)"
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "get_asset_profile".to_string(),
            description: "Get detailed profile for a specific asset including sector, \
                country, market cap, and other fundamental data."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "dataSource": {
                        "type": "string",
                        "description": "Data source identifier (e.g., 'YAHOO')"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Ticker symbol (e.g., 'AAPL')"
                    }
                },
                "required": ["dataSource", "symbol"]
            }),
        },
        Tool {
            name: "get_market_data".to_string(),
            description: "Get current market data including major index values \
                and market status."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_benchmarks".to_string(),
            description: "Get benchmark performance data for major market indices.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}
