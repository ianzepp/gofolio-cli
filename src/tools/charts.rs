//! Chart tool implementations.
//!
//! These tools accept data and store chart definitions that the TUI
//! renders inline in the chat panel using ratatui's Sparkline and BarChart widgets.

/// Parse a chart_sparkline tool call and return a confirmation + chart data.
pub fn sparkline(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let title = input["title"].as_str().unwrap_or("Chart");
    let data = input["data"].as_array().ok_or("missing 'data' array")?;

    let values: Vec<f64> = data.iter().filter_map(|v| v.as_f64()).collect();

    if values.is_empty() {
        return Err("'data' array is empty or contains no numbers".to_string());
    }

    Ok(serde_json::json!({
        "chart_type": "sparkline",
        "title": title,
        "data": values,
        "points": values.len(),
        "min": values.iter().cloned().fold(f64::INFINITY, f64::min),
        "max": values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    }))
}

/// Parse a chart_bar tool call and return a confirmation + chart data.
pub fn bar(input: &serde_json::Value) -> Result<serde_json::Value, String> {
    let title = input["title"].as_str().unwrap_or("Chart");
    let bars = input["bars"].as_array().ok_or("missing 'bars' array")?;

    if bars.is_empty() {
        return Err("'bars' array is empty".to_string());
    }

    let mut labels = Vec::new();
    let mut values = Vec::new();

    for bar in bars {
        let label = bar["label"].as_str().unwrap_or("?").to_string();
        let value = bar["value"]
            .as_f64()
            .ok_or_else(|| format!("bar '{}' missing numeric 'value'", label))?;
        labels.push(label);
        values.push(value);
    }

    Ok(serde_json::json!({
        "chart_type": "bar",
        "title": title,
        "labels": labels,
        "values": values,
        "count": labels.len(),
    }))
}
