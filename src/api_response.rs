//! Shared response envelopes. Server refusals remain typed and never include payload contents.
use anyhow::{ensure, Context, Result};
use serde_json::Value;
/// Explicit numeric server refusal, suitable for documented domain-specific handling.
#[derive(Debug)]
pub struct ServerError(pub i64);
impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Máy chủ từ chối thao tác ({})", self.0)
    }
}
impl std::error::Error for ServerError {}
/// Parse a wire integer without treating missing, floating or invalid values as success.
pub fn integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.trim().parse().ok())
}
/// Read a typed refusal through anyhow context instead of matching error text.
pub fn server_code(error: &anyhow::Error) -> Option<i64> {
    error.downcast_ref::<ServerError>().map(|error| error.0)
}
/// Peel up to four envelopes while preserving a configurable JSON-string budget.
/// # Errors
/// Rejects malformed codes, missing data, oversized encoded bodies and missing required acknowledgement.
pub(crate) fn data(mut value: Value, string_budget: usize, require_ack: bool) -> Result<Value> {
    let mut acknowledged = false;
    for depth in 0..=4 {
        if let Value::String(text) = value {
            ensure!(
                text.len() <= string_budget,
                "Phản hồi máy chủ vượt giới hạn"
            );
            value = serde_json::from_str(&text).context("Không đọc được JSON phản hồi máy chủ")?;
        }
        if value.get("error_code").is_none() {
            ensure!(!require_ack || acknowledged, "Thiếu xác nhận máy chủ");
            return Ok(value);
        }
        ensure!(depth < 4, "Phản hồi máy chủ có quá nhiều lớp");
        let code = value
            .get("error_code")
            .and_then(integer)
            .context("Mã trả về máy chủ không hợp lệ")?;
        if code != 0 {
            return Err(ServerError(code).into());
        }
        acknowledged = true;
        value = value
            .as_object_mut()
            .and_then(|object| object.remove("data"))
            .context("Thiếu dữ liệu phản hồi máy chủ")?;
    }
    anyhow::bail!("Phản hồi máy chủ có quá nhiều lớp")
}
/// Parse an authenticated operation response; a bare object cannot acknowledge a mutation.
/// # Errors
/// Returns explicit server refusals or malformed/oversized envelopes without raw payload details.
pub fn checked_data(value: Value) -> Result<Value> {
    data(value, 4 * 1024 * 1024, true)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn nested_refusal_is_typed_and_missing_ack_never_becomes_success() {
        let error=checked_data(json!({"error_code":0,"data":"{\"error_code\":240,\"data\":{\"secret\":\"private-text\"}}"})).unwrap_err();
        assert_eq!(server_code(&error), Some(240));
        assert!(!error.to_string().contains("private-text"));
        assert!(checked_data(json!({"data":{}})).is_err());
        assert!(checked_data(json!({"error_code":null,"data":{}})).is_err());
        assert_eq!(
            checked_data(json!({"error_code":"0","data":null})).unwrap(),
            Value::Null
        );
        assert!(data(json!("123456789"), 8, false).is_err());
    }
}
