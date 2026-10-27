/// NEP-297 log line. `data` is always a one-element array, matching the
/// event shape used by NEP-141 and NEP-171.
pub fn event_json(event: &str, data: serde_json::Value) -> String {
    let payload = serde_json::json!({
        "standard": "fe2vi",
        "version": "1.0.0",
        "event": event,
        "data": [data],
    });
    format!("EVENT_JSON:{payload}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_line_is_nep297() {
        let line = event_json("game_created", serde_json::json!({"id": "g1"}));
        assert!(line.starts_with("EVENT_JSON:"));
        let value: serde_json::Value = serde_json::from_str(&line["EVENT_JSON:".len()..]).unwrap();
        assert_eq!(value["standard"], "fe2vi");
        assert_eq!(value["event"], "game_created");
        assert_eq!(value["data"][0]["id"], "g1");
    }
}
