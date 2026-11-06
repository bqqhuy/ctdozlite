//! Account-scoped notification snapshots, independent of viewport rendering.
use std::sync::Mutex;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    pub owner: String,
    pub thread_id: String,
    pub message_id: String,
    pub title: String,
    pub body: String,
    pub avatar: Option<String>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub owner: String,
    pub count: usize,
    pub enabled: bool,
    pub sound: bool,
    pub sequence: u64,
    pub notice: Option<Notice>,
}
static STATE: Mutex<Snapshot> = Mutex::new(Snapshot {
    owner: String::new(),
    count: 0,
    enabled: true,
    sound: false,
    sequence: 0,
    notice: None,
});
static ACTIVATION: Mutex<Option<Notice>> = Mutex::new(None);
pub struct Avatar {
    pub owner: String,
    pub url: String,
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
}
static AVATARS: Mutex<Vec<std::sync::Arc<Avatar>>> = Mutex::new(Vec::new());
pub fn cache_avatar(owner: String, url: String, image: eframe::egui::ColorImage) {
    if image.size[0] > 256 || image.size[1] > 256 {
        return;
    }
    let bgra = image
        .pixels
        .iter()
        .flat_map(|p| [p.b(), p.g(), p.r(), 255])
        .collect();
    if let Ok(mut avatars) = AVATARS.lock() {
        if avatars.len() >= 8 {
            avatars.remove(0);
        }
        avatars.push(std::sync::Arc::new(Avatar {
            owner,
            url,
            width: image.size[0],
            height: image.size[1],
            bgra,
        }));
    }
}
pub fn avatar(notice: &Notice) -> Option<std::sync::Arc<Avatar>> {
    let url = notice.avatar.as_ref()?;
    AVATARS
        .lock()
        .ok()?
        .iter()
        .rev()
        .find(|a| a.owner == notice.owner && a.url == *url)
        .cloned()
}
pub fn account(owner: &str) {
    if let Ok(mut s) = STATE.lock() {
        if s.owner != owner {
            let sequence = s.sequence.wrapping_add(1);
            let enabled = s.enabled;
            let sound = s.sound;
            *s = Snapshot {
                owner: owner.into(),
                sequence,
                enabled,
                sound,
                ..Default::default()
            };
        }
    }
}
pub fn configure(enabled: bool, sound: bool) {
    if let Ok(mut s) = STATE.lock() {
        s.enabled = enabled;
        s.sound = sound;
        if !enabled {
            s.notice = None;
        }
    }
}
pub fn count(owner: &str, count: usize) {
    account(owner);
    if let Ok(mut s) = STATE.lock() {
        if s.owner == owner {
            s.count = count;
        }
    }
}
pub fn publish(notice: Notice, count: usize) {
    account(&notice.owner);
    if let Ok(mut s) = STATE.lock() {
        if !s.enabled || s.owner != notice.owner {
            return;
        }
        s.count = count;
        s.sequence = s.sequence.wrapping_add(1);
        s.notice = Some(notice);
    }
}
/// Retract account previews after a privacy relock, including a pending activation/avatar.
pub fn clear_previews(owner: &str) {
    if let Ok(mut state) = STATE.lock() {
        if state.owner == owner {
            state.notice = None;
            state.sequence = state.sequence.wrapping_add(1);
        }
    }
    if let Ok(mut activation) = ACTIVATION.lock() {
        if activation
            .as_ref()
            .is_some_and(|notice| notice.owner == owner)
        {
            *activation = None;
        }
    }
    if let Ok(mut avatars) = AVATARS.lock() {
        avatars.retain(|avatar| avatar.owner != owner);
    }
}
pub fn snapshot() -> Snapshot {
    STATE.lock().map(|s| s.clone()).unwrap_or_default()
}
pub fn activate(notice: Notice) {
    if snapshot().owner == notice.owner {
        if let Ok(mut slot) = ACTIVATION.lock() {
            *slot = Some(notice);
        }
    }
}
pub fn take_activation() -> Option<Notice> {
    ACTIVATION.lock().ok().and_then(|mut s| s.take())
}
pub fn badge(count: usize) -> String {
    match count {
        0 => String::new(),
        1..=5 => count.to_string(),
        _ => "5+".into(),
    }
}

/// Monotonic replacement state retains the clicked card until its exit completes.
#[derive(Default)]
pub struct Popup {
    pub current: Option<Notice>,
    pub pending: Option<Notice>,
    pub shown_at: Option<std::time::Instant>,
    pub exit_at: Option<std::time::Instant>,
    pub seen: u64,
}
impl Popup {
    pub fn update(&mut self, s: &Snapshot, now: std::time::Instant, background: bool) {
        if !s.enabled
            || s.owner.is_empty()
            || !background
            || self.current.as_ref().is_some_and(|n| n.owner != s.owner)
        {
            self.current = None;
            self.pending = None;
            self.shown_at = None;
            self.exit_at = None;
        }
        if s.sequence != self.seen {
            self.seen = s.sequence;
            if s.notice.is_none() {
                self.close();
            }
            if s.enabled && background {
                if let Some(n) = &s.notice {
                    if self.current.is_some() {
                        self.pending = Some(n.clone());
                        if self.exit_at.is_none() {
                            self.exit_at = Some(now);
                        }
                    } else {
                        self.current = Some(n.clone());
                        self.shown_at = Some(now);
                    }
                }
            }
        }
        if self.exit_at.is_some_and(|t| {
            now.saturating_duration_since(t) >= std::time::Duration::from_millis(150)
        }) {
            self.current = self.pending.take();
            self.exit_at = None;
            self.shown_at = self.current.as_ref().map(|_| now);
        }
        if self.exit_at.is_none()
            && self.shown_at.is_some_and(|t| {
                now.saturating_duration_since(t) >= std::time::Duration::from_secs(10)
            })
        {
            self.exit_at = Some(now);
        }
    }
    /// Keep a hovered card readable without delaying a newer pending card.
    pub fn hold(&mut self, now: std::time::Instant) {
        if self.current.is_some() && self.pending.is_none() && self.exit_at.is_none() {
            self.shown_at = Some(now);
        }
    }
    pub fn close(&mut self) {
        self.current = None;
        self.pending = None;
        self.shown_at = None;
        self.exit_at = None;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_preserves_click_target_and_restarts_expiry() {
        let now = std::time::Instant::now();
        let mut p = Popup::default();
        let mut s = Snapshot {
            owner: "owner".into(),
            enabled: true,
            sequence: 1,
            notice: Some(Notice {
                owner: "owner".into(),
                thread_id: "a".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        p.update(&s, now, true);
        s.sequence = 2;
        s.notice.as_mut().unwrap().thread_id = "b".into();
        p.update(&s, now + std::time::Duration::from_secs(2), true);
        assert_eq!(p.current.as_ref().unwrap().thread_id, "a");
        p.update(&s, now + std::time::Duration::from_millis(2160), true);
        assert_eq!(p.current.as_ref().unwrap().thread_id, "b");
        p.update(&s, now + std::time::Duration::from_secs(5), true);
        assert!(p.current.is_some());
        assert!(p.exit_at.is_none());
    }
    #[test]
    fn display_cap_does_not_change_the_count() {
        assert_eq!(badge(0), "");
        assert_eq!(badge(5), "5");
        assert_eq!(badge(1000), "5+");
    }
}

#[cfg(test)]
mod lifetime_tests {
    use super::*;
    #[test]
    fn popup_lasts_ten_seconds_and_hover_extends_reading_time() {
        use std::time::{Duration, Instant};
        let now = Instant::now();
        let mut p = Popup::default();
        let s = Snapshot {
            owner: "owner".into(),
            enabled: true,
            sequence: 1,
            notice: Some(Notice {
                owner: "owner".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        p.update(&s, now, true);
        p.update(&s, now + Duration::from_secs(9), true);
        assert!(p.exit_at.is_none());
        p.hold(now + Duration::from_secs(9));
        p.update(&s, now + Duration::from_secs(11), true);
        assert!(p.exit_at.is_none());
        p.update(&s, now + Duration::from_secs(19), true);
        assert!(p.exit_at.is_some());
        p.update(&s, now + Duration::from_millis(19160), true);
        assert!(p.current.is_none());
    }
}

#[cfg(test)]
mod privacy_tests {
    use super::*;
    #[test]
    fn clearing_the_notice_sequence_retracts_current_and_pending_cards() {
        let now = std::time::Instant::now();
        let notice = Notice {
            owner: "owner".into(),
            thread_id: "12".into(),
            message_id: "1".into(),
            title: "Tên".into(),
            body: "Nội dung".into(),
            avatar: None,
        };
        let mut snapshot = Snapshot {
            owner: "owner".into(),
            enabled: true,
            sequence: 1,
            notice: Some(notice),
            ..Default::default()
        };
        let mut popup = Popup::default();
        popup.update(&snapshot, now, true);
        assert!(popup.current.is_some());
        snapshot.sequence = 2;
        snapshot.notice = None;
        popup.update(&snapshot, now, true);
        assert!(popup.current.is_none());
        assert!(popup.pending.is_none());
    }
}
