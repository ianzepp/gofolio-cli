use crate::api::{ApiError, GhostfolioClient};

use super::query_params;

fn normalize_activity_range(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    let mapped = match normalized.as_str() {
        "1d" | "1w" | "1m" | "1y" | "5y" | "max" | "mtd" | "wtd" | "ytd" => {
            normalized
        }
        "today" | "last day" | "past day" | "24h" => "1d".to_string(),
        "last week" | "past week" | "one week" => "1w".to_string(),
        "last month" | "past month" | "one month" | "recent" | "recent transactions" => {
            "1m".to_string()
        }
        "last year" | "past year" | "one year" => "1y".to_string(),
        "this week" => "wtd".to_string(),
        "this month" => "mtd".to_string(),
        "this year" | "year to date" => "ytd".to_string(),
        "all time" | "since inception" => "max".to_string(),
        _ => {
            if trimmed.len() == 4 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                trimmed.to_string()
            } else {
                return None;
            }
        }
    };

    Some(mapped)
}

pub fn to_toon(data: &serde_json::Value) -> Option<String> {
    let root = data.as_object()?;
    let activities = root.get("activities")?.as_array()?;

    let headers = [
        "id", "date", "type", "symbol", "name", "qty", "unitPrice", "value", "baseValue",
        "currency", "fee", "account", "platform", "tags", "comment",
    ];

    let mut out = String::new();
    out.push_str(&format!(
        "activities[{}]{{{}}}:\n",
        activities.len(),
        headers.join(",")
    ));

    for activity in activities {
        let row = [
            scalar(activity.get("id")),
            compact_date(scalar(activity.get("date"))),
            scalar(activity.get("type")),
            scalar(path(activity, &["SymbolProfile", "symbol"])),
            scalar(path(activity, &["SymbolProfile", "name"])),
            scalar(activity.get("quantity")),
            scalar(activity.get("unitPrice")),
            scalar(activity.get("value")),
            scalar(activity.get("valueInBaseCurrency")),
            scalar(activity.get("currency")),
            scalar(activity.get("feeInBaseCurrency").or(activity.get("fee"))),
            scalar(path(activity, &["account", "name"])),
            scalar(path(activity, &["account", "platform", "name"])),
            tags(activity.get("tags")),
            scalar(activity.get("comment")),
        ];

        let encoded = row.iter().map(|v| quote_cell(v)).collect::<Vec<_>>().join(",");
        out.push_str("  ");
        out.push_str(&encoded);
        out.push('\n');
    }

    if let Some(count) = root.get("count").and_then(|v| v.as_u64()) {
        out.push_str(&format!("count: {count}\n"));
    }

    Some(out)
}

fn path<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in keys {
        current = current.get(*key)?;
    }
    Some(current)
}

fn scalar(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::Null) | None => String::new(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Number(n)) => n.to_string(),
        Some(serde_json::Value::Bool(b)) => b.to_string(),
        Some(other) => other.to_string(),
    }
}

fn compact_date(raw: String) -> String {
    raw.split('T').next().unwrap_or(&raw).to_string()
}

fn tags(value: Option<&serde_json::Value>) -> String {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return String::new();
    };

    let names = arr
        .iter()
        .filter_map(|item| {
            item.get("name")
                .and_then(|v| v.as_str())
                .or_else(|| item.as_str())
        })
        .collect::<Vec<_>>();

    names.join("|")
}

fn quote_cell(cell: &str) -> String {
    if cell.is_empty()
        || cell.contains(',')
        || cell.contains('"')
        || cell.contains('\n')
        || cell.contains('\r')
        || cell.starts_with(' ')
        || cell.ends_with(' ')
    {
        format!("\"{}\"", cell.replace('"', "\\\""))
    } else {
        cell.to_string()
    }
}

pub async fn list_activities(
    client: &GhostfolioClient,
    input: &serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let params = query_params(
        input,
        &[
            "accounts",
            "assetClasses",
            "dataSource",
            "symbol",
            "tags",
            "sortColumn",
            "sortDirection",
            "skip",
            "take",
        ],
    );
    let mut params = params;
    if let Some(raw_range) = input.get("range").and_then(|value| value.as_str())
        && let Some(range) = normalize_activity_range(raw_range)
    {
        params.push(("range".to_string(), range));
    }

    let refs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    client.get_with_query("/api/v1/order", &refs).await
}

#[cfg(test)]
mod tests {
    use super::{normalize_activity_range, to_toon};

    #[test]
    fn keeps_supported_range_values() {
        assert_eq!(normalize_activity_range("1m"), Some("1m".to_string()));
        assert_eq!(normalize_activity_range("ytd"), Some("ytd".to_string()));
    }

    #[test]
    fn maps_common_natural_language_aliases() {
        assert_eq!(
            normalize_activity_range("last month"),
            Some("1m".to_string())
        );
        assert_eq!(normalize_activity_range("this week"), Some("wtd".to_string()));
        assert_eq!(
            normalize_activity_range("recent transactions"),
            Some("1m".to_string())
        );
    }

    #[test]
    fn accepts_year_ranges_and_rejects_unknown_values() {
        assert_eq!(normalize_activity_range("2025"), Some("2025".to_string()));
        assert_eq!(normalize_activity_range("foobar"), None);
        assert_eq!(normalize_activity_range(""), None);
    }

    #[test]
    fn renders_activity_payload_as_toon() {
        let payload = serde_json::json!({
            "activities": [
                {
                    "id": "a1",
                    "date": "2026-02-01T00:00:00.000Z",
                    "type": "BUY",
                    "quantity": 2,
                    "unitPrice": 100.5,
                    "value": 201.0,
                    "valueInBaseCurrency": 201.0,
                    "currency": "USD",
                    "feeInBaseCurrency": 1.2,
                    "comment": "test",
                    "SymbolProfile": { "symbol": "AAPL", "name": "Apple Inc." },
                    "account": { "name": "Brokerage", "platform": { "name": "IBKR" } },
                    "tags": [{ "name": "core" }, { "name": "tech" }]
                }
            ],
            "count": 1
        });

        let toon = to_toon(&payload).expect("expected toon");
        assert!(toon.contains("activities[1]{id,date,type,symbol,name,qty,unitPrice,value,baseValue,currency,fee,account,platform,tags,comment}:"));
        assert!(toon.contains("a1,2026-02-01,BUY,AAPL,Apple Inc.,2,100.5,201.0,201.0,USD,1.2,Brokerage,IBKR,core|tech,test"));
        assert!(toon.contains("count: 1"));
    }
}
