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
    use super::normalize_activity_range;

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
}
