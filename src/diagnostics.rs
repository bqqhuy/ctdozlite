//! Bounded diagnostic previews; secrets and message bodies are never displayed.
use serde_json::Value;
fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "password",
        "passw",
        "zpw",
        "secret",
        "token",
        "cookie",
        "imei",
        "session",
        "sessid",
        "authorization",
        "phone",
    ]
    .iter()
    .any(|part| key.contains(part))
        || [
            "key",
            "params",
            "message",
            "msg",
            "content",
            "body",
            "name",
            "displayname",
            "dname",
            "fromd",
            "error_message",
            "text",
            "caption",
            "zaloname",
            "groupname",
            "nickname",
            "display_name",
        ]
        .contains(&key.as_str())
}
fn scrub(value: &mut Value, depth: usize) {
    if depth >= 12 {
        *value = Value::String("<nested payload omitted>".into());
        return;
    }
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if sensitive(key) {
                    *value = Value::String("<redacted>".into());
                } else if key == "data" && value.is_string() {
                    if let Ok(mut inner) =
                        serde_json::from_str::<Value>(value.as_str().unwrap_or_default())
                    {
                        scrub(&mut inner, depth + 1);
                        *value = inner;
                    } else {
                        *value = Value::String("<opaque payload>".into());
                    }
                } else {
                    scrub(value, depth + 1);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub(value, depth + 1);
            }
        }
        Value::String(text)
            if (text.starts_with('{') || text.starts_with('[')) && text.len() <= 65536 =>
        {
            if let Ok(mut inner) = serde_json::from_str::<Value>(text) {
                scrub(&mut inner, depth + 1);
                *value = inner;
            }
        }
        _ => {}
    }
}
/// Redact JSON (including nested JSON strings) or encoded forms before truncation.
pub(crate) fn preview(raw: &str) -> String {
    let result = if raw.len() > 65536 {
        format!("<payload omitted: {} bytes>", raw.len())
    } else if let Ok(mut value) = serde_json::from_str::<Value>(raw) {
        scrub(&mut value, 0);
        value.to_string()
    } else if raw.contains('=') && !raw.contains(['{', '\n', ' ']) {
        let fields: serde_json::Map<String, Value> =
            reqwest::Url::parse(&format!("http://diagnostic.invalid/?{raw}"))
                .ok()
                .into_iter()
                .flat_map(|u| {
                    u.query_pairs()
                        .map(|(k, v)| (k.into_owned(), Value::String(v.into_owned())))
                        .collect::<Vec<_>>()
                })
                .collect();
        let mut value = Value::Object(fields);
        scrub(&mut value, 0);
        value.to_string()
    } else {
        format!("<opaque payload: {} bytes>", raw.len())
    };
    crate::session::truncate(&result, 16384)
}
/// Keep technical trace lines unless they contain embedded JSON or secret markers.
pub(crate) fn trace(raw: &str) -> String {
    if serde_json::from_str::<Value>(raw).is_ok() {
        return preview(raw);
    }
    let lowered = raw.to_ascii_lowercase();
    if raw.contains('{')
        || [
            "token=",
            "session=",
            "session_id=",
            "sessid=",
            "zpw_",
            "imei=",
            "cookie=",
            "password=",
            "params=",
        ]
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        format!("<sensitive trace omitted: {} bytes>", raw.len())
    } else {
        crate::session::truncate(raw, 16384)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_signaling_and_form_secrets_never_enter_preview() {
        let output = preview(
            r#"{"command":401,"data":"{\"zpw_sek\":\"fixture-secret\",\"status\":0}","payload":{"imei":"fixture-device","session":"fixture-session","message":"private-body"}}"#,
        );
        for secret in [
            "fixture-secret",
            "fixture-device",
            "fixture-session",
            "private-body",
        ] {
            assert!(!output.contains(secret));
        }
        assert!(output.contains("401"));
        assert!(output.contains("status"));
        assert!(!preview("params=fixture-secret&status=0").contains("fixture-secret"));
        assert!(!trace("relay token=fixture-secret").contains("fixture-secret"));
        assert_eq!(trace("rtp packets=10"), "rtp packets=10");
    }
}
