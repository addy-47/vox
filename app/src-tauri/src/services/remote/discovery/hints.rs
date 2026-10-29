//! LAN address hint manipulation and formatting.
//!
//! Spec: `docs/specs/remote-control-spec.md` §5.7 (Address Hints).

use std::net::IpAddr;

// ─── Constants & Type Aliases ────────────────────────────────────────────────

// ─── Data Structures ─────────────────────────────────────────────────────────

// ─── Trait Implementations ───────────────────────────────────────────────────

// ─── Inherent Implementations ────────────────────────────────────────────────

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Extracts the stored `{"addr": "ip:port"}` strings from an opaque hints
/// array. Unknown hint kinds are skipped here: they round-trip through
/// persistence but are not dialable today.
pub fn lan_hint_addrs(hints: &[serde_json::Value]) -> Vec<String> {
    hints
        .iter()
        .filter_map(|h| h.get("addr").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .collect()
}

/// Builds an opaque hint JSON value for a reachable LAN address.
/// Spec §5.7 `HintSource`: `Qr | Mdns | Observed`.
pub fn lan_hint(addr: &str, source: &str, last_ok_at_ms: Option<i64>) -> serde_json::Value {
    let mut hint = serde_json::json!({ "addr": addr, "source": source });
    if let Some(ts) = last_ok_at_ms {
        hint["last_ok_at"] = serde_json::json!(ts);
    }
    hint
}

/// Updates the hint array with a freshly observed candidate at the front,
/// preserving existing hints (including forward-compatible opaque hint kinds).
pub fn update_observed_hint(existing: &[serde_json::Value], addr: &str) -> Vec<serde_json::Value> {
    let mut updated = Vec::new();
    updated.push(lan_hint(
        addr,
        "Observed",
        Some(chrono::Utc::now().timestamp_millis()),
    ));
    for h in existing {
        if h.get("addr").and_then(|v| v.as_str()) != Some(addr) {
            updated.push(h.clone());
        }
    }
    updated
}

/// Turns an `ip[:port]` candidate into a `ws://ip[:port]` URL with the
/// WebSocket default port applied when the candidate carries no port.
pub(crate) fn dial_url(addr: &str, default_port: u16) -> String {
    if addr.starts_with("ws://") || addr.starts_with("wss://") {
        return addr.to_string();
    }
    if addr
        .rsplit_once(':')
        .map(|(_, p)| p.contains('.'))
        .unwrap_or(true)
    {
        // No (or a bare) IPv6-port colon split — ambiguous IPv6 is out of v1
        // scope (LAN v1 is IPv4); treat as portless and append the default.
        format!("ws://{addr}:{default_port}")
    } else {
        format!("ws://{addr}")
    }
}

/// Collects the machine's non-loopback local IPs for the mDNS A records.
/// Desktop and Android both enumerate via libc; no platform gate needed.
pub(crate) fn local_ips() -> Vec<String> {
    let mut ips = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|ifa| match ifa.ip() {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => Some(v4.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    ips.dedup();
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_lan_addrs_from_opaque_hints() {
        let hints = vec![
            serde_json::json!({ "addr": "192.168.29.221:7700" }),
            serde_json::json!({ "kind": "relay", "relay": "future.com" }),
            serde_json::json!({ "addr": "10.0.0.5:7700", "source": "Mdns" }),
        ];
        assert_eq!(
            lan_hint_addrs(&hints),
            vec![
                "192.168.29.221:7700".to_string(),
                "10.0.0.5:7700".to_string()
            ]
        );
    }

    #[test]
    fn lan_hint_round_trips_with_source_and_timestamp() {
        let hint = lan_hint("10.0.0.5:7700", "Mdns", Some(1723));
        assert_eq!(hint["addr"], "10.0.0.5:7700");
        assert_eq!(hint["source"], "Mdns");
        assert_eq!(hint["last_ok_at"], 1723);
        let round = serde_json::from_str::<Vec<serde_json::Value>>(
            &serde_json::to_string(&vec![hint]).unwrap(),
        )
        .unwrap();
        assert_eq!(lan_hint_addrs(&round).len(), 1);
    }

    #[test]
    fn updates_observed_hint_preserving_other_hints() {
        let existing = vec![
            serde_json::json!({ "kind": "relay", "relay": "future.com" }),
            serde_json::json!({ "addr": "192.168.1.1:7700", "source": "Qr" }),
        ];
        let updated = update_observed_hint(&existing, "192.168.1.200:7700");
        assert_eq!(updated.len(), 3);
        assert_eq!(updated[0]["addr"], "192.168.1.200:7700");
        assert_eq!(updated[0]["source"], "Observed");
        assert_eq!(updated[1]["kind"], "relay");
        assert_eq!(updated[2]["addr"], "192.168.1.1:7700");
    }

    #[test]
    fn dial_url_passes_through_ws_schemes() {
        assert_eq!(dial_url("ws://10.0.0.5:7700", 7700), "ws://10.0.0.5:7700");
    }

    #[test]
    fn dial_url_appends_default_port_when_missing() {
        assert_eq!(dial_url("192.168.1.7", 7700), "ws://192.168.1.7:7700");
        assert_eq!(dial_url("192.168.1.7:9000", 7700), "ws://192.168.1.7:9000");
    }
}
