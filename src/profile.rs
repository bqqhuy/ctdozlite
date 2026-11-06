//! Own-profile editing follows PC updateProfileMe, never the legacy registration/password API.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
/// Editable fields and display avatar for the authenticated account only.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub uid: String,
    pub business: bool,
    pub name: String,
    pub gender: Option<i64>,
    pub dob: Option<String>,
    pub avatar: Option<String>,
}
impl Snapshot {
    /// Decode me-v2's profile wrapper; unknown gender/birthday stays unknown.
    /// # Errors
    /// Rejects a different account or malformed name/identity.
    pub fn parse(data: Value, owner: &str) -> Result<Self> {
        let profile = data.get("profile").context("Thiếu hồ sơ tài khoản")?;
        let uid = profile
            .get("userId")
            .or_else(|| profile.get("uid"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| value.as_u64().map(|id| id.to_string()))
            })
            .context("Thiếu định danh hồ sơ")?;
        ensure!(
            !owner.is_empty() && uid == owner,
            "Hồ sơ không thuộc tài khoản hiện tại"
        );
        let name = ["zaloName", "displayName"]
            .iter()
            .find_map(|key| {
                profile
                    .get(*key)
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty() && name.len() <= 1024)
            })
            .context("Thiếu tên hồ sơ")?
            .to_owned();
        let gender = profile
            .get("gender")
            .and_then(crate::api_response::integer)
            .filter(|value| matches!(value, 0 | 1));
        let dob = profile.get("dob").and_then(|value| {
            if let Some(text) = value.as_str() {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
                    return Some(date.format("%Y-%m-%d").to_string());
                }
            }
            let ts = crate::api_response::integer(value).filter(|value| *value != 0)?;
            // Birthdays before 1970 use a negative epoch; message timestamp normalization rejects those.
            let millis = if ts.unsigned_abs() < 10_000_000_000 {
                ts.saturating_mul(1000)
            } else {
                ts
            };
            chrono::DateTime::from_timestamp_millis(millis).map(|date| {
                date.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d")
                    .to_string()
            })
        });
        let avatar = profile
            .get("avatar")
            .or_else(|| profile.get("avt"))
            .and_then(Value::as_str)
            .filter(|url| url.len() <= 8192)
            .map(str::to_owned);
        let business = profile
            .pointer("/bizInfo/pkgId")
            .and_then(crate::api_response::integer)
            .is_some_and(|id| id != 0);
        Ok(Self {
            business,
            uid,
            name,
            gender,
            dob,
            avatar,
        })
    }
}
/// A complete explicit user edit, displayed for confirmation before sending.
#[derive(Clone, Debug)]
pub enum Action {
    Read,
    Save {
        name: String,
        gender: i64,
        dob: String,
    },
}
/// Build the official name/gender/ISO-date payload, preserving the registration endpoint boundary.
/// # Errors
/// Rejects invalid names, unsupported gender and impossible/future birthdays.
pub fn update_params(name: &str, gender: i64, dob: &str) -> Result<Value> {
    let name = name.trim();
    ensure!(
        (2..=40).contains(&name.chars().count()) && !name.chars().any(char::is_control),
        "Tên hồ sơ phải có từ 2 đến 40 ký tự"
    );
    ensure!(matches!(gender, 0 | 1), "Giới tính chưa được hỗ trợ");
    let date = chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
        .context("Ngày sinh phải có dạng YYYY-MM-DD")?;
    let minimum =
        chrono::NaiveDate::from_ymd_opt(1900, 1, 1).context("Không khởi tạo được mốc ngày sinh")?;
    ensure!(
        date >= minimum && date <= chrono::Local::now().date_naive(),
        "Ngày sinh không hợp lệ"
    );
    Ok(
        json!({"name":name,"gender":gender,"dob":date.format("%Y-%m-%d").to_string(),"language":"vi"}),
    )
}
async fn read(s: &Session) -> Result<Snapshot> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let params = s.encoded_params(&json!({"avatar_size":160,"imei":s.imei}))?;
    let raw = s
        .get(&format!(
            "{}/api/social/profile/me-v2?{}&params={params}&os=8",
            s.domains.profile,
            s.common_params()
        ))
        .await?;
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận hồ sơ từ máy chủ"
    );
    Snapshot::parse(crate::api_response::checked_data(raw)?, s.account_key())
}
/// Read or save once and validate the result through me-v2 readback.
/// # Errors
/// Returns account, transport, validation and unconfirmed changes without automatic retry.
pub async fn execute(s: &Session, action: &Action) -> Result<Snapshot> {
    let body = match action {
        Action::Read => None,
        Action::Save { name, gender, dob } => Some(update_params(name, *gender, dob)?),
    };
    let before = read(s).await?;
    let Some(body) = body else {
        return Ok(before);
    };
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    let params = s.encoded_params(&body)?;
    let raw = s
        .post_form(
            &format!(
                "{}/api/social/profile/update?{}",
                s.domains.profile,
                s.common_params()
            ),
            format!("params={params}"),
        )
        .await?;
    let raw = s.maybe_decrypt(raw, s.session_key_bytes()).await;
    ensure!(
        raw.get("error_code").is_some(),
        "Thiếu xác nhận cập nhật hồ sơ"
    );
    crate::api_response::checked_data(raw)?;
    let after = read(s).await?;
    ensure!(
        after.name == body["name"].as_str().unwrap_or("")
            && after.gender == body["gender"].as_i64()
            && after.dob.as_deref() == body["dob"].as_str(),
        "Chưa xác nhận máy chủ đã áp dụng hồ sơ; tải lại trước khi thử tiếp"
    );
    Ok(after)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn own_profile_rejects_other_accounts_and_invalid_birthdays_without_defaulting_unknown_fields()
    {
        let raw =
            json!({"profile":{"userId":"123","zaloName":"Tên 😀","dob":"2000-02-29","gender":0}});
        let profile = Snapshot::parse(raw.clone(), "123").unwrap();
        assert_eq!(profile.dob.as_deref(), Some("2000-02-29"));
        assert!(Snapshot::parse(raw, "456").is_err());
        assert!(update_params("Tên", 0, "2001-02-29").is_err());
        assert!(update_params("Tên", 2, "2000-01-01").is_err());
        assert_eq!(
            update_params("Tên 😀", 0, "2000-02-29").unwrap()["dob"],
            "2000-02-29"
        );
        assert!(
            Snapshot::parse(json!({"profile":{"userId":"123","zaloName":"Tên"}}), "123")
                .unwrap()
                .gender
                .is_none()
        );
    }
}
