use super::types::Tool;

pub fn all_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "get_portfolio_summary".to_string(),
            description: "Get a detailed summary of the user's entire portfolio. Returns accounts \
                (id, name, currency, balance, valueInBaseCurrency), holdings keyed by symbol \
                (quantity, marketPrice, allocationInPercentage, investment, valueInBaseCurrency, \
                netPerformance, netPerformancePercent, grossPerformance, assetClass, assetSubClass, \
                currency, countries[], sectors[]), platforms, and a summary object with \
                currentNetWorth, totalInvestment, netPerformance, netPerformancePercent, cash, \
                dividendInBaseCurrency, fees, totalBuy, totalSell, activityCount. \
                Also includes hasError boolean if any calculation failed."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range for performance calculations. Defaults to 'max'.",
                        "enum": ["1d", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "get_holdings".to_string(),
            description: "Get all current portfolio holdings as an array. Each holding includes: \
                symbol, name, currency, dataSource, assetClass, assetSubClass, quantity, \
                marketPrice, allocationInPercentage, investment, valueInBaseCurrency, \
                valueInPercentage, netPerformance, netPerformancePercent, \
                grossPerformance, grossPerformancePercent, countries[], sectors[], tags[]. \
                Supports filtering by accounts, asset classes, search query, and holding type."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Free-text search filter on holding names/symbols (case-insensitive)"
                    },
                    "holdingType": {
                        "type": "string",
                        "description": "Filter by holding type",
                        "enum": ["ACCOUNT", "CASH", "COMMODITY", "CRYPTO", "ETF", "MUTUALFUND", "PRIVATE_EQUITY"]
                    },
                    "range": {
                        "type": "string",
                        "description": "Time range for performance calculations. Defaults to 'max'.",
                        "enum": ["1d", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    },
                    "accounts": {
                        "type": "string",
                        "description": "Comma-separated account UUIDs to filter by"
                    },
                    "assetClasses": {
                        "type": "string",
                        "description": "Comma-separated asset classes to filter (e.g. 'EQUITY,FIXED_INCOME')"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "get_holding_detail".to_string(),
            description: "Get detailed information about a specific holding the user owns. \
                Returns: averagePrice, quantity, marketPrice, marketPriceMax, marketPriceMin, \
                value, dateOfFirstActivity, activitiesCount, dividendInBaseCurrency, \
                dividendYieldPercent, feeInBaseCurrency, investmentInBaseCurrencyWithCurrencyEffect, \
                netPerformance, netPerformancePercent, grossPerformance, grossPerformancePercent, \
                historicalData[] (date, marketPrice, netPerformance, netWorth, totalInvestment), \
                performances.allTimeHigh (date, performancePercent), \
                SymbolProfile (name, currency, assetClass, assetSubClass, countries[], sectors[], \
                isin, url), tags[]. Use dataSource and symbol from get_holdings or search_assets."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "dataSource": {
                        "type": "string",
                        "description": "Data source identifier from the holding (e.g. 'YAHOO', 'COINGECKO', 'MANUAL')"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Ticker symbol from the holding (e.g. 'AAPL', 'VWCE.DE', 'bitcoin')"
                    }
                },
                "required": ["dataSource", "symbol"]
            }),
        },
        Tool {
            name: "get_performance".to_string(),
            description: "Get portfolio performance over a time range. Returns: \
                performance object with currentNetWorth, currentValueInBaseCurrency, \
                netPerformance, netPerformancePercentage, \
                netPerformancePercentageWithCurrencyEffect, totalInvestment, \
                totalInvestmentValueWithCurrencyEffect, annualizedPerformancePercent. \
                Also returns chart[] with time-series data (date, netPerformance, netWorth, \
                totalInvestment, totalAccountBalance, investmentValueWithCurrencyEffect), \
                and firstOrderDate. Supports filtering by accounts, asset classes, and symbol."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range for performance data. Defaults to 'max'.",
                        "enum": ["1d", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    },
                    "accounts": {
                        "type": "string",
                        "description": "Comma-separated account UUIDs to filter by"
                    },
                    "assetClasses": {
                        "type": "string",
                        "description": "Comma-separated asset classes to filter (e.g. 'EQUITY,FIXED_INCOME')"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "get_dividends".to_string(),
            description: "Get dividend income data for the portfolio. Returns: \
                dividends[] array where each item has date (ISO string) and \
                investment (dividend amount in base currency). \
                Use groupBy to aggregate by month or year for a summary view."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range for dividend data. Defaults to 'max'.",
                        "enum": ["1d", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    },
                    "groupBy": {
                        "type": "string",
                        "description": "Aggregate dividends by time period",
                        "enum": ["month", "year"]
                    },
                    "accounts": {
                        "type": "string",
                        "description": "Comma-separated account UUIDs to filter by"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "get_investments".to_string(),
            description: "Get investment contribution data over time. Returns: \
                investments[] array where each item has date (ISO string) and \
                investment (total invested amount in base currency), plus \
                streaks object with currentStreak and longestStreak \
                (consecutive months/years with investment contributions). \
                Use groupBy to aggregate by month or year."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range for investment data. Defaults to 'max'.",
                        "enum": ["1d", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    },
                    "groupBy": {
                        "type": "string",
                        "description": "Aggregate investments by time period",
                        "enum": ["month", "year"]
                    },
                    "accounts": {
                        "type": "string",
                        "description": "Comma-separated account UUIDs to filter by"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "list_activities".to_string(),
            description: "List portfolio activities (trades, dividends, fees, interest, etc.). \
                Returns: activities[] and count. Each activity has: id, date, type \
                (BUY, SELL, DIVIDEND, INTEREST, FEE, LIABILITY, ITEM), quantity, unitPrice, \
                fee, feeInBaseCurrency, value, valueInBaseCurrency, currency, comment, isDraft, \
                SymbolProfile (symbol, name, dataSource, currency, assetClass, assetSubClass), \
                account (name, currency, platform). Supports pagination, sorting, and filtering."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "range": {
                        "type": "string",
                        "description": "Time range filter for activities. Defaults to 'max'.",
                        "enum": ["1d", "1w", "1m", "1y", "5y", "max", "mtd", "wtd", "ytd"]
                    },
                    "accounts": {
                        "type": "string",
                        "description": "Comma-separated account UUIDs to filter by"
                    },
                    "assetClasses": {
                        "type": "string",
                        "description": "Comma-separated asset classes to filter"
                    },
                    "sortColumn": {
                        "type": "string",
                        "description": "Column to sort by (e.g. 'date', 'type', 'symbol')"
                    },
                    "sortDirection": {
                        "type": "string",
                        "description": "Sort direction",
                        "enum": ["asc", "desc"]
                    },
                    "skip": {
                        "type": "integer",
                        "description": "Number of records to skip (pagination offset)"
                    },
                    "take": {
                        "type": "integer",
                        "description": "Number of records to return (pagination limit)"
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "list_accounts".to_string(),
            description: "List all investment accounts with aggregated values. Returns: \
                accounts[] with id, name, currency, balance, balanceInBaseCurrency, \
                valueInBaseCurrency, allocationInPercentage, activitiesCount, \
                dividendInBaseCurrency, interestInBaseCurrency, isExcluded, \
                platform (name). Also returns totals: totalBalanceInBaseCurrency, \
                totalValueInBaseCurrency, totalDividendInBaseCurrency, \
                totalInterestInBaseCurrency, activitiesCount."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_account_balances".to_string(),
            description: "Get the balance history for a specific account as a time series. \
                Returns: balances[] where each entry has id, accountId, date (ISO string), \
                value (in account currency), and valueInBaseCurrency. \
                Use the account id from list_accounts."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Account UUID (from list_accounts response)"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "search_assets".to_string(),
            description: "Search for financial assets by name or ticker symbol. \
                Returns: items[] where each item has symbol, name, currency, \
                dataSource (e.g. 'YAHOO', 'COINGECKO', 'MANUAL'), \
                assetClass (EQUITY, FIXED_INCOME, COMMODITY, LIQUIDITY, REAL_ESTATE), \
                assetSubClass (STOCK, ETF, BOND, CRYPTOCURRENCY, MUTUALFUND, PRECIOUS_METAL), \
                and dataProviderInfo. Use the returned dataSource and symbol values \
                when calling get_holding_detail or get_asset_profile."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search term — company name (e.g. 'Apple'), ticker (e.g. 'AAPL'), or ISIN"
                    },
                    "includeIndices": {
                        "type": "string",
                        "description": "Set to 'true' to include benchmark indices in results. Defaults to 'false'.",
                        "enum": ["true", "false"]
                    }
                },
                "required": ["query"]
            }),
        },
        Tool {
            name: "get_asset_profile".to_string(),
            description: "Get the fundamental profile for any asset (not just holdings). \
                Returns: assetProfile with symbol, name, currency, dataSource, \
                assetClass, assetSubClass, isActive, isin, cusip, figi, url, \
                countries[] (code, name, weight), sectors[] (name, weight), \
                holdings[] (name, weight — for ETFs/funds), activitiesCount, \
                watchedByCount, createdAt, updatedAt. \
                Also returns marketData[] with historical date and marketPrice entries. \
                Use dataSource and symbol from search_assets or get_holdings."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "dataSource": {
                        "type": "string",
                        "description": "Data source identifier (e.g. 'YAHOO', 'COINGECKO', 'MANUAL')"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Asset symbol (e.g. 'AAPL', 'VWCE.DE', 'bitcoin')"
                    }
                },
                "required": ["dataSource", "symbol"]
            }),
        },
        Tool {
            name: "get_market_data".to_string(),
            description: "Get current market sentiment data. Returns: \
                fearAndGreedIndex object with CRYPTOCURRENCIES and STOCKS entries, \
                each containing dataSource, symbol, currency, and marketPrice \
                (the current Fear & Greed Index value 0-100). \
                Does not return individual stock prices — use get_holdings or \
                get_asset_profile for that."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_benchmarks".to_string(),
            description: "Get benchmark performance data for major market indices. \
                Returns: benchmarks[] where each has dataSource, symbol, name, \
                marketCondition ('ALL_TIME_HIGH', 'BEAR_MARKET', or 'NEUTRAL_MARKET'), \
                performances.allTimeHigh (date, performancePercent — how far below ATH), \
                trend50d (direction: 'UP'/'DOWN', value), \
                trend200d (direction: 'UP'/'DOWN', value). \
                Useful for market context and comparing portfolio performance to indices."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}
