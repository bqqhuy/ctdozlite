//! Bank Account (STK) Suite and Transfer APIs (Zalo Parity Gói 4).
//!
//! Handles bank configuration catalog, user saved bank accounts,
//! and sending bank card cards into 1-1 and group chats.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::session::Session;

fn deserialize_string_or_number<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrNumberVisitor;
    impl<'de> serde::de::Visitor<'de> for StringOrNumberVisitor {
        type Value = String;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or number")
        }
        fn visit_str<E>(self, v: &str) -> std::result::Result<String, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }
        fn visit_i64<E>(self, v: i64) -> std::result::Result<String, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }
        fn visit_u64<E>(self, v: u64) -> std::result::Result<String, E>
        where
            E: serde::de::Error,
        {
            Ok(v.to_string())
        }
    }
    deserializer.deserialize_any(StringOrNumberVisitor)
}

fn deserialize_opt_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_string_or_number(deserializer).or_else(|_| Ok(String::new()))
}

/// Bank institution information from `/api/transfer/conf` (CmdId 15309).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankInfo {
    #[serde(deserialize_with = "deserialize_string_or_number")]
    pub bin: String,
    #[serde(default)]
    pub logo: String,
    pub name: String,
    #[serde(default, alias = "nameEng")]
    pub name_eng: String,
    #[serde(default, alias = "shortName")]
    pub short_name: String,
    #[serde(default, alias = "searchKeyWord")]
    pub search_key_word: String,
}

/// User saved bank card from `/api/transfer/list` (CmdId 15298).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BankAccount {
    #[serde(
        default,
        alias = "accountId",
        alias = "id",
        deserialize_with = "deserialize_opt_string_or_number"
    )]
    pub account_id: String,
    #[serde(alias = "bank_bin", deserialize_with = "deserialize_string_or_number")]
    pub bin: String,
    #[serde(
        alias = "bankNumber",
        alias = "account_number",
        alias = "accountNumber",
        deserialize_with = "deserialize_string_or_number"
    )]
    pub bank_number: String,
    #[serde(alias = "holderName", alias = "account_holder")]
    pub holder_name: String,
    #[serde(default, alias = "isDefault")]
    pub is_default: bool,
    #[serde(default, alias = "bankName")]
    pub bank_name: String,
    #[serde(default)]
    pub logo: String,
}

/// Validate bank account number (Regex: `^[A-Za-z0-9]{1,19}$`).
pub fn validate_bank_number(number: &str) -> bool {
    let trimmed = number.trim();
    !trimmed.is_empty() && trimmed.len() <= 19 && trimmed.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Normalize and validate cardholder name (Regex: `^(?=.{5,50}$)[A-Za-z0-9]+(?: [A-Za-z0-9]+)*$`).
pub fn normalize_and_validate_holder_name(name: &str) -> Option<String> {
    let folded = crate::search::fold(name.trim()).to_ascii_uppercase();
    let words: Vec<&str> = folded.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let normalized = words.join(" ");
    if normalized.len() >= 5
        && normalized.len() <= 50
        && normalized
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == ' ')
    {
        Some(normalized)
    } else {
        None
    }
}

/// GET /api/transfer/conf (CmdId 15309).
/// Pure GET request, no encrypted `params=` query.
pub async fn fetch_bank_conf(s: &Session) -> Result<Vec<BankInfo>> {
    let url = format!(
        "{}/api/transfer/conf?{}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let data_obj = crate::api_response::checked_data(data)?;
    let mut banks = Vec::new();
    if let Some(arr) = data_obj.get("banks").and_then(|b| b.as_array()) {
        for b in arr {
            if let Ok(info) = serde_json::from_value::<BankInfo>(b.clone()) {
                banks.push(info);
            }
        }
    }
    Ok(banks)
}

/// GET /api/transfer/list (CmdId 15298).
pub async fn fetch_bank_accounts(
    s: &Session,
    page: u32,
    limit: u32,
) -> Result<(Vec<BankAccount>, u32, bool)> {
    let body = json!({
        "page": page,
        "limit": limit,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/transfer/list?{}&params={params}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.get(&url).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let data_obj = crate::api_response::checked_data(data)?;
    let total = data_obj.get("total").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
    let has_more = data_obj
        .get("hasMore")
        .and_then(|h| h.as_bool())
        .unwrap_or(false);
    let mut accounts = Vec::new();
    if let Some(arr) = data_obj
        .get("myBanks")
        .or_else(|| data_obj.get("banks"))
        .and_then(|mb| mb.as_array())
    {
        for a in arr {
            match serde_json::from_value::<BankAccount>(a.clone()) {
                Ok(acc) => accounts.push(acc),
                Err(e) => {
                    crate::logger::log_error(
                        "bank_account::fetch",
                        &format!("Bỏ qua tài khoản ngân hàng do lỗi schema: {e}, payload: {a:?}"),
                    );
                }
            }
        }
    }
    Ok((accounts, total, has_more))
}

/// POST /api/transfer/create (CmdId 15306).
pub async fn create_bank_account(
    s: &Session,
    bin: &str,
    bank_number: &str,
    holder_name: &str,
) -> Result<BankAccount> {
    let body = json!({
        "bin": bin,
        "bank_number": bank_number,
        "holder_name": holder_name,
        "language": "vi",
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/transfer/create?{}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    let data_obj = crate::api_response::checked_data(data)?;
    if let Ok(mut acc) = serde_json::from_value::<BankAccount>(data_obj.clone()) {
        if acc.account_id.is_empty() {
            if let Some(id_val) = data_obj
                .get("account_id")
                .or_else(|| data_obj.get("accountId"))
                .or_else(|| data_obj.get("id"))
            {
                acc.account_id = crate::api_response::string_or_integer(id_val).unwrap_or_default();
            }
        }
        if !acc.account_id.is_empty() {
            return Ok(acc);
        }
    }
    let acc_id = data_obj
        .get("account_id")
        .or_else(|| data_obj.get("accountId"))
        .or_else(|| data_obj.get("id"))
        .and_then(crate::api_response::string_or_integer)
        .ok_or_else(|| {
            anyhow::anyhow!("Phản hồi máy chủ tạo tài khoản ngân hàng thiếu mã ID hợp lệ")
        })?;

    Ok(BankAccount {
        account_id: acc_id,
        bin: bin.to_string(),
        bank_number: bank_number.to_string(),
        holder_name: holder_name.to_string(),
        is_default: false,
        bank_name: data_obj
            .get("bank_name")
            .or_else(|| data_obj.get("bankName"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        logo: data_obj
            .get("logo")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

/// POST /api/transfer/update (CmdId 15307).
pub async fn update_bank_account(
    s: &Session,
    account_id: &str,
    bin: &str,
    bank_number: &str,
    holder_name: &str,
) -> Result<()> {
    let body = json!({
        "account_id": account_id,
        "bin": bin,
        "bank_number": bank_number,
        "holder_name": holder_name,
        "language": "vi",
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/transfer/update?{}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    crate::api_response::checked_data(data)?;
    Ok(())
}

/// POST /api/transfer/delete (CmdId 15308).
pub async fn delete_bank_account(s: &Session, account_id: &str, is_default: bool) -> Result<()> {
    let body = json!({
        "account_id": account_id,
        "is_default": is_default,
        "language": "vi",
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/transfer/delete?{}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    crate::api_response::checked_data(data)?;
    Ok(())
}

/// POST /api/transfer/card (CmdId 15220).
#[allow(clippy::too_many_arguments)]
pub async fn send_bank_card(
    s: &Session,
    to_id: &str,
    is_group: bool,
    bin: &str,
    bank_number: &str,
    holder_name: &str,
    client_id: u64,
    ts: u64,
) -> Result<()> {
    crate::chat::require_plaintext_send(s, to_id, is_group)?;
    anyhow::ensure!(
        !crate::e2ee_integration::check_thread_e2ee(s.account_key(), to_id)?,
        "Hội thoại yêu cầu E2EE; không được gửi thẻ ngân hàng qua endpoint thường"
    );
    let dest_uid = if is_group {
        to_id
            .trim_start_matches("group_")
            .trim_start_matches('g')
            .trim_start_matches('G')
            .to_string()
    } else {
        to_id.to_string()
    };
    let dest_type = if is_group { 1 } else { 0 };
    let body = json!({
        "binBank": bin,
        "numAccBank": bank_number,
        "nameAccBank": holder_name,
        "cliMsgId": client_id,
        "tsMsg": ts,
        "destUid": dest_uid,
        "destType": dest_type,
    });
    let params = s.encoded_params(&body)?;
    let url = format!(
        "{}/api/transfer/card?{}",
        crate::endpoints::Z_INSTANT_MSG,
        s.common_params()
    );
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let data = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    crate::api_response::checked_data(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_bank_number() {
        assert!(validate_bank_number("19036789999"));
        assert!(validate_bank_number("0123456789012345678")); // 19 digits
        assert!(!validate_bank_number("")); // empty
        assert!(!validate_bank_number("12345678901234567890")); // 20 digits > 19
        assert!(!validate_bank_number("1234-5678")); // hyphen not allowed
    }

    #[test]
    fn test_normalize_and_validate_holder_name() {
        let name = "Nguyễn Văn A";
        let normalized = normalize_and_validate_holder_name(name);
        assert_eq!(normalized, Some("NGUYEN VAN A".to_string()));

        let invalid_short = "A B";
        assert_eq!(normalize_and_validate_holder_name(invalid_short), None);

        let valid_english = "JOHN DOE";
        assert_eq!(
            normalize_and_validate_holder_name(valid_english),
            Some("JOHN DOE".to_string())
        );
    }

    #[tokio::test]
    async fn test_send_bank_card_fails_closed_when_thread_requires_e2ee() {
        let uin = format!("bank_e2ee_test_{}", crate::param_cipher::random_hex(6));
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(crate::session::LoginInfo {
            uin: uin.clone(),
            zpw_enk: "enk".into(),
            zpw_sek: "sek".into(),
            display_name: "Test User".into(),
            send2me_id: String::new(),
            avatar: String::new(),
        });

        let direct_peer = "direct_e2ee_peer_1";
        crate::e2ee_integration::remember_e2ee_requirement(&uin, direct_peer).unwrap();

        // Direct send to E2EE peer must fail before any network request
        let err_direct = send_bank_card(
            &session,
            direct_peer,
            false,
            "970436",
            "1234567890",
            "NGUYEN VAN A",
            1001,
            123456789,
        )
        .await
        .unwrap_err();

        assert!(
            err_direct.to_string().contains("E2EE"),
            "Expected E2EE rejection, got: {err_direct}"
        );

        let group_peer = "group_e2ee_peer_1";
        crate::e2ee_integration::remember_e2ee_requirement(&uin, group_peer).unwrap();

        // Group send to E2EE marked group must fail before any network request
        let err_group = send_bank_card(
            &session,
            group_peer,
            true,
            "970436",
            "1234567890",
            "NGUYEN VAN A",
            1002,
            123456789,
        )
        .await
        .unwrap_err();

        assert!(
            err_group.to_string().contains("E2EE"),
            "Expected E2EE rejection for group, got: {err_group}"
        );

        // Clean up test keys
        let _ = crate::e2ee_integration::remove_e2ee_requirement(&uin, direct_peer);
        let _ = crate::e2ee_integration::remove_e2ee_requirement(&uin, group_peer);
    }
}
