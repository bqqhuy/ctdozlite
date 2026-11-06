//! Server hidden-chat membership and an in-memory PIN gate; local history is not encrypted.
use crate::session::Session;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::Mutex,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};
/// This public view contains neither the PIN nor its protocol verifier.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub ready: bool,
    pub has_pin: bool,
    pub pin_known: bool,
    pub unlocked: bool,
    pub ids: HashSet<String>,
}
impl Snapshot {
    /// Unknown membership fails closed; locked server-hidden chats cannot be searched or opened.
    pub fn allows(&self, id: &str, group: bool) -> bool {
        self.ready
            && (self.unlocked
                || self.ids.is_empty()
                || crate::server_pins::identity(id, group)
                    .ok()
                    .is_some_and(|id| !self.ids.contains(&id)))
    }
    pub fn export_allowed(&self) -> bool {
        self.ready && (self.unlocked || self.ids.is_empty())
    }
}
#[derive(Default)]
struct State {
    owner: String,
    view: Snapshot,
    verifier: Zeroizing<String>,
    failures: u32,
    blocked_until: Option<Instant>,
}
/// Owned by one session runtime and shared with its worker snapshots only.
#[derive(Default)]
pub struct Vault {
    enforced: std::sync::atomic::AtomicBool,
    state: Mutex<State>,
    pub operations: tokio::sync::Mutex<()>,
}
impl Vault {
    pub fn enable(&self) {
        self.enforced
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn enforced(&self) -> bool {
        self.enforced.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn view(&self, owner: &str) -> Snapshot {
        self.state
            .lock()
            .ok()
            .filter(|state| state.owner == owner)
            .map(|state| state.view.clone())
            .unwrap_or_default()
    }
    pub fn invalidate(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.view.revision = state.view.revision.wrapping_add(1);
            state.view.ready = false;
            state.view.unlocked = false;
        }
    }
    pub fn lock(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.view.revision = state.view.revision.wrapping_add(1);
            state.view.unlocked = false;
        }
    }
    fn replace(&self, owner: &str, mut view: Snapshot, verifier: Zeroizing<String>) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Không đọc được khóa hội thoại"))?;
        let unchanged = state.owner == owner
            && aws_lc_rs::constant_time::verify_slices_are_equal(
                state.verifier.as_bytes(),
                verifier.as_bytes(),
            )
            .is_ok();
        view.revision = state.view.revision.wrapping_add(1);
        view.unlocked = unchanged && state.view.unlocked;
        if !unchanged {
            state.failures = 0;
            state.blocked_until = None;
        }
        state.owner = owner.into();
        state.view = view;
        state.verifier = verifier;
        Ok(())
    }
    fn unlock(&self, owner: &str, pin: &str) -> Result<()> {
        validate_pin(pin)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Không đọc được khóa hội thoại"))?;
        ensure!(
            state.owner == owner && state.view.ready && state.view.pin_known && state.view.has_pin,
            "Chưa tải được trạng thái PIN"
        );
        ensure!(
            !state
                .blocked_until
                .is_some_and(|until| until > Instant::now()),
            "Đã nhập sai nhiều lần; đợi một phút rồi thử lại"
        );
        state.view.revision = state.view.revision.wrapping_add(1);
        let digest = Zeroizing::new(crate::crypto::md5_hex(pin));
        if aws_lc_rs::constant_time::verify_slices_are_equal(
            digest.as_bytes(),
            state.verifier.as_bytes(),
        )
        .is_err()
        {
            state.failures = state.failures.saturating_add(1);
            state.view.unlocked = false;
            if state.failures >= 5 {
                state.blocked_until = Some(Instant::now() + Duration::from_secs(60));
                state.failures = 0;
            }
            anyhow::bail!("PIN không đúng");
        }
        state.failures = 0;
        state.blocked_until = None;
        state.view.unlocked = true;
        Ok(())
    }
}
/// Secret-bearing commands intentionally do not implement Debug or serialization.
pub enum Action {
    Read,
    Lock,
    Unlock(Zeroizing<String>),
    SetPin {
        old: Zeroizing<String>,
        new: Zeroizing<String>,
    },
    SetHidden {
        target: String,
        group: bool,
        hidden: bool,
    },
}
impl Action {
    pub fn mutation(&self) -> bool {
        matches!(self, Self::SetPin { .. } | Self::SetHidden { .. })
    }
}
fn validate_pin(pin: &str) -> Result<()> {
    ensure!(
        pin.len() == 4 && pin.bytes().all(|byte| byte.is_ascii_digit()),
        "PIN phải gồm 4 chữ số"
    );
    Ok(())
}
fn parse(mut data: Value) -> Result<(Snapshot, Zeroizing<String>)> {
    let pin_known = data.get("pin").is_some();
    let pin = data
        .get_mut("pin")
        .map(Value::take)
        .unwrap_or_else(|| Value::String(String::new()));
    let Value::String(pin) = pin else {
        anyhow::bail!("Định dạng PIN máy chủ chưa được hỗ trợ");
    };
    let mut pin = Zeroizing::new(pin);
    ensure!(
        pin.is_empty() || (pin.len() == 32 && pin.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "Định dạng PIN máy chủ chưa được hỗ trợ"
    );
    pin.make_ascii_lowercase();
    let rows = data
        .get("threads")
        .and_then(Value::as_array)
        .context("Thiếu danh sách hội thoại ẩn")?;
    ensure!(rows.len() <= 20_000, "Danh sách hội thoại ẩn quá lớn");
    let mut ids = HashSet::new();
    for row in rows {
        let id = row
            .get("thread_id")
            .and_then(|id| {
                id.as_str()
                    .map(str::to_owned)
                    .or_else(|| id.as_u64().map(|id| id.to_string()))
            })
            .context("Thiếu định danh hội thoại ẩn")?;
        let group = match row.get("is_group").and_then(crate::api_response::integer) {
            Some(0) => false,
            Some(1) => true,
            _ => anyhow::bail!("Thiếu loại hội thoại ẩn"),
        };
        ensure!(
            ids.insert(crate::server_pins::identity(&id, group)?),
            "Hội thoại ẩn bị trùng"
        );
    }
    ensure!(
        !pin_known || !pin.is_empty() || ids.is_empty(),
        "Danh sách ẩn có dữ liệu nhưng thiếu PIN; cần tải lại"
    );
    Ok((
        Snapshot {
            revision: 0,
            ready: true,
            has_pin: !pin.is_empty() || !ids.is_empty(),
            pin_known,
            unlocked: false,
            ids,
        },
        pin,
    ))
}
async fn call(s: &Session, path: &str, body: Option<Value>, post: bool) -> Result<Value> {
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    // The response includes a protocol verifier: never send it through developer exchange dumps.
    let mut private = s.clone();
    private.dump = false;
    private.dump_raw = false;
    private.verbose = false;
    let mut url = format!(
        "{}{path}?{}",
        private.domains.convers_api,
        private.common_params()
    );
    let params = body
        .map(|mut body| {
            let result = private.encoded_params(&body);
            if let Some(Value::String(pin)) = body.get_mut("new_pin") {
                pin.zeroize();
            }
            result
        })
        .transpose()?;
    let raw = if post {
        private
            .post_form(&url, format!("params={}", params.context("Thiếu tham số")?))
            .await?
    } else {
        if let Some(params) = params {
            url.push_str("&params=");
            url.push_str(&params);
        }
        private.get(&url).await?
    };
    let data = crate::api_response::checked_data(
        private
            .maybe_decrypt(raw, private.session_key_bytes())
            .await,
    )?;
    ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
    Ok(data)
}
async fn refresh(s: &Session) -> Result<()> {
    let (view, verifier) = parse(
        call(
            s,
            "/api/hiddenconvers/get-all",
            Some(json!({"imei":s.imei})),
            false,
        )
        .await?,
    )?;
    s.hidden.replace(s.account_key(), view, verifier)
}
/// Serialize reads and mutations so a stale pre-mutation response cannot restore an old PIN.
/// # Errors
/// Unavailable state never bypasses the gate; uncertain mutations are not repeated.
pub async fn execute(s: &Session, action: &Action) -> Result<Snapshot> {
    ensure!(
        s.is_active() && !s.account_key().is_empty(),
        "Phiên tài khoản đã thay đổi"
    );
    let _guard = s.hidden.operations.lock().await;
    let result:Result<()>=async {
        match action {
            Action::Read=>refresh(s).await?,
            Action::Lock=>s.hidden.lock(),
            Action::Unlock(pin)=>{refresh(s).await?;s.hidden.unlock(s.account_key(),pin)?;}
            Action::SetPin {old,new}=>{
                validate_pin(new)?;refresh(s).await?;
                ensure!(s.hidden.view(s.account_key()).pin_known,"Máy chủ chưa cung cấp trạng thái PIN; chưa thể đặt hoặc đổi PIN");
                if s.hidden.view(s.account_key()).has_pin {s.hidden.unlock(s.account_key(),old)?;}
                let mut digest=crate::crypto::md5_hex(new);
                let response=call(s,"/api/hiddenconvers/update-pin",Some(json!({"new_pin":digest,"imei":s.imei})),false).await;digest.zeroize();response?;
                s.hidden.lock();refresh(s).await?;s.hidden.unlock(s.account_key(),new)?;
            }
            Action::SetHidden {target,group,hidden}=>{
                refresh(s).await?;let view=s.hidden.view(s.account_key());ensure!(view.has_pin && view.unlocked,"Mở khóa PIN trước khi thay đổi hội thoại ẩn");
                let identity=crate::server_pins::identity(target,*group)?;let item=json!({"thread_id":&identity[1..],"is_group":i64::from(*group)});
                let (add,del)=if *hidden {(vec![item],vec![])}else{(vec![],vec![item])};
                call(s,"/api/hiddenconvers/add-remove",Some(json!({"add_threads":serde_json::to_string(&add)?,"del_threads":serde_json::to_string(&del)?,"imei":s.imei})),true).await?;
                refresh(s).await?;ensure!(s.hidden.view(s.account_key()).ids.contains(&identity)==*hidden,"Chưa xác nhận trạng thái hội thoại ẩn; tải lại trước khi thử tiếp");
            }
        }Ok(())
    }.await;
    if let Err(error) = result {
        if action.mutation() {
            s.hidden.invalidate();
        }
        return Err(error);
    }
    let view = s.hidden.view(s.account_key());
    if view.ready
        && matches!(
            action,
            Action::Read | Action::SetHidden { .. } | Action::SetPin { .. }
        )
    {
        save_membership(s, &view).await;
    }
    Ok(view)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifier_is_memory_only_and_account_scoped() {
        let (view,pin)=parse(json!({"pin":"81dc9bdb52d04dc20036dbd8313ed055","threads":[{"thread_id":"12","is_group":0},{"thread_id":34,"is_group":1}]})).unwrap();
        let vault = Vault::default();
        vault.replace("owner", view, pin).unwrap();
        assert!(!vault.view("owner").allows("12", false));
        assert!(vault.view("owner").allows("99", false));
        assert!(!vault.view("other").allows("99", false));
        assert!(vault.unlock("owner", "0000").is_err());
        vault.unlock("owner", "1234").unwrap();
        assert!(vault.view("owner").allows("g34", true));
        vault.lock();
        assert!(!vault.view("owner").export_allowed());
        vault.invalidate();
        assert!(!vault.view("owner").allows("99", false));
    }
    #[test]
    fn malformed_membership_cannot_be_an_empty_unlocked_list() {
        for value in [
            json!({}),
            json!({"pin":"","threads":[{"thread_id":"12","is_group":0}]}),
            json!({"pin":"x","threads":[]}),
        ] {
            assert!(parse(value).is_err());
        }
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    fn fixture() -> (Snapshot, Zeroizing<String>) {
        parse(json!({"pin":"81dc9bdb52d04dc20036dbd8313ed055","threads":[{"thread_id":"12","is_group":0}]})).unwrap()
    }
    #[test]
    fn pin_change_locks_and_advances_revision_without_losing_membership() {
        let vault = Vault::default();
        let (view, pin) = fixture();
        vault.replace("owner", view, pin).unwrap();
        vault.unlock("owner", "1234").unwrap();
        let old = vault.view("owner");
        let (view, _) = fixture();
        vault
            .replace(
                "owner",
                view,
                Zeroizing::new(crate::crypto::md5_hex("5678")),
            )
            .unwrap();
        let new = vault.view("owner");
        assert!(new.revision > old.revision);
        assert!(!new.unlocked);
        assert!(new.ids.contains("u12"));
        assert!(vault.unlock("owner", "1234").is_err());
        vault.unlock("owner", "5678").unwrap();
    }
    #[test]
    fn five_failed_attempts_cannot_be_bypassed_by_a_normal_refresh() {
        let vault = Vault::default();
        let (view, pin) = fixture();
        vault.replace("owner", view, pin).unwrap();
        for _ in 0..5 {
            assert!(vault.unlock("owner", "0000").is_err());
            let (view, pin) = fixture();
            vault.replace("owner", view, pin).unwrap();
        }
        assert!(vault.unlock("owner", "1234").is_err());
        assert!(!vault.view("owner").unlocked);
    }
    #[test]
    fn snapshots_contain_only_typed_membership_and_ui_flags() {
        let (view, _) = fixture();
        let text = format!("{view:?}");
        assert!(!text.contains("81dc9bdb"));
        assert!(!view.allows("12", false));
        assert!(view.allows("12", true));
        assert!(!Snapshot::default().allows("99", false));
        assert!(Snapshot {
            ready: true,
            ..Default::default()
        }
        .allows("fixture", false));
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MembershipCache {
    version: u8,
    owner: String,
    has_pin: bool,
    ids: HashSet<String>,
}
fn cache_path(s: &Session) -> std::path::PathBuf {
    s.cache_path().with_file_name("hidden_membership.json")
}
fn decode_cache(bytes: &[u8], owner: &str) -> Result<Snapshot> {
    let cache: MembershipCache = serde_json::from_slice(bytes)?;
    ensure!(
        cache.version == 1 && cache.owner == owner && !owner.is_empty(),
        "Danh sách ẩn lưu thuộc tài khoản khác"
    );
    ensure!(cache.ids.len() <= 20_000, "Danh sách ẩn lưu quá lớn");
    for id in &cache.ids {
        ensure!(
            (id.starts_with('u') || id.starts_with('g'))
                && crate::server_pins::identity(id, id.starts_with('g'))? == *id,
            "ID ẩn lưu không hợp lệ"
        );
    }
    Ok(Snapshot {
        ready: true,
        has_pin: cache.has_pin || !cache.ids.is_empty(),
        pin_known: false,
        ids: cache.ids,
        ..Default::default()
    })
}
/// Restore account-bound membership before network refresh. No cached verifier can unlock a chat.
/// # Errors
/// Corrupt/foreign caches fail closed; existing live state always takes precedence.
pub async fn restore_cached(s: &Session) -> Result<Option<Snapshot>> {
    let _guard = s.hidden.operations.lock().await;
    if s.hidden.view(s.account_key()).ready {
        return Ok(None);
    }
    let account = s.clone();
    let path = cache_path(s);
    let loaded = tokio::task::spawn_blocking(move || -> Result<Option<Snapshot>> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = crate::limits::read_file(&path, 4 * 1024 * 1024)?;
        Ok(Some(decode_cache(&bytes, account.account_key())?))
    })
    .await??;
    if let Some(view) = loaded {
        ensure!(s.is_active(), "Phiên tài khoản đã thay đổi");
        s.hidden
            .replace(s.account_key(), view, Zeroizing::new(String::new()))?;
        return Ok(Some(s.hidden.view(s.account_key())));
    }
    Ok(None)
}
async fn save_membership(s: &Session, view: &Snapshot) {
    let account = s.clone();
    let cache = MembershipCache {
        version: 1,
        owner: s.account_key().to_owned(),
        has_pin: view.has_pin,
        ids: view.ids.clone(),
    };
    let path = cache_path(s);
    let result = tokio::task::spawn_blocking(move || -> Result<()> {
        ensure!(account.is_active(), "Phiên đã đóng");
        let bytes = serde_json::to_vec(&cache)?;
        if crate::limits::read_file(&path, 4 * 1024 * 1024).is_ok_and(|old| old == bytes) {
            return Ok(());
        }
        crate::storage::atomic_write(&path, &bytes)
    })
    .await;
    if !matches!(result, Ok(Ok(()))) {
        crate::logger::log_error(
            "hidden membership",
            "Cannot persist membership; PIN has not been saved",
        );
    }
}
#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn offline_cache_contains_no_pin_and_cannot_unlock() {
        let bytes = serde_json::to_vec(&MembershipCache {
            version: 1,
            owner: "owner".into(),
            has_pin: true,
            ids: HashSet::from(["u12".into(), "g34".into()]),
        })
        .unwrap();
        let view = decode_cache(&bytes, "owner").unwrap();
        assert!(!view.unlocked);
        assert!(!view.pin_known);
        assert!(view.allows("99", false));
        assert!(!view.allows("12", false));
        assert!(decode_cache(&bytes, "other").is_err());
        assert!(decode_cache(
            br#"{"version":1,"owner":"owner","has_pin":false,"ids":["invalid"]}"#,
            "owner"
        )
        .is_err());
    }
    #[test]
    fn an_absent_verifier_does_not_block_verified_nonhidden_chats() {
        let (view, _) = parse(json!({"threads":[{"thread_id":"12","is_group":0}]})).unwrap();
        assert!(view.ready && !view.pin_known);
        assert!(view.allows("99", false));
        assert!(!view.allows("12", false));
    }
}
