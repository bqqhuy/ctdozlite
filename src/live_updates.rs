//! Coalesce live delivery independently of whether the desktop viewport is rendering.
use crate::chat::Thread;
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone)]
struct Baseline {
    read_ids: std::collections::HashSet<String>,
    unread: crate::unread::UnreadState,
}

/// Coalesced, account-worker-owned live delivery mailbox.
#[derive(Default)]
pub struct LiveUpdates {
    // One short ownership lock covers snapshots only, never disk/network/render work.
    pending: Mutex<(HashMap<String, Thread>, bool)>,
    muted: Mutex<std::collections::HashSet<String>>,
    baselines: Mutex<HashMap<String, Baseline>>,
    alerts: Mutex<(HashMap<String, u128>, bool)>,
    notice: Mutex<Option<crate::desktop_notice::Notice>>,
    counts: Mutex<HashMap<String, usize>>,
    names: Mutex<HashMap<String, (String, Option<String>)>>,
}
impl LiveUpdates {
    pub(crate) fn publish(&self, mut threads: Vec<Thread>) -> bool {
        for thread in &mut threads {
            crate::message_view::remove_call_logs(thread);
        }
        let batches: Vec<_> = {
            let seeds = self.baselines.lock().unwrap_or_else(|e| e.into_inner());
            threads
                .into_iter()
                .map(|thread| {
                    let seed = seeds.get(&thread.id).cloned();
                    (thread, seed)
                })
                .collect()
        };
        let muted = self.muted.lock().unwrap_or_else(|e| e.into_inner()).clone();
        {
            let mut alerts = self.alerts.lock().unwrap_or_else(|e| e.into_inner());
            for (thread, seed) in &batches {
                let seen = thread
                    .unread
                    .last_seen_id
                    .as_deref()
                    .and_then(|id| id.parse::<u128>().ok())
                    .unwrap_or(0)
                    .max(
                        seed.as_ref()
                            .and_then(|b| b.unread.last_seen_id.as_deref())
                            .and_then(|id| id.parse::<u128>().ok())
                            .unwrap_or(0),
                    );
                let previous = alerts.0.get(&thread.id).copied().unwrap_or(0).max(seen);
                let mut latest = previous;
                for message in &thread.messages {
                    if let Ok(id) = message.id.parse::<u128>() {
                        if id > previous
                            && !message.is_outgoing
                            && message.kind != crate::message_model::MessageKind::System
                            && message.unread_policy.counts_unread()
                            && !thread.unread.is_muted()
                            && !seed
                                .as_ref()
                                .is_some_and(|baseline| baseline.unread.is_muted())
                            && !muted.contains(&thread.id)
                        {
                            alerts.1 = true;
                            let cached = self
                                .names
                                .lock()
                                .ok()
                                .and_then(|names| names.get(&thread.id).cloned());
                            let title = cached
                                .as_ref()
                                .map(|p| p.0.clone())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| thread.name.clone());
                            let avatar = thread.avatar.clone().or_else(|| cached.and_then(|p| p.1));
                            let preview = Thread {
                                is_group: thread.is_group,
                                messages: vec![message.clone()],
                                unread: thread.unread.clone(),
                                ..Default::default()
                            };
                            if let Ok(mut slot) = self.notice.lock() {
                                *slot = Some(crate::desktop_notice::Notice {
                                    title,
                                    body: crate::message_view::preview(&preview),
                                    avatar,
                                    thread_id: thread.id.clone(),
                                    message_id: message.id.clone(),
                                    owner: String::new(),
                                });
                            }
                        }
                        latest = latest.max(id);
                    }
                }
                alerts.0.insert(thread.id.clone(), latest);
            }
        }
        let mut state = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        for (mut incoming, seed) in batches {
            incoming.unread.advance_seen("0");
            incoming.unread.observe(&incoming.messages);
            incoming.unread.fresh = true;
            let entry = state
                .0
                .entry(incoming.id.clone())
                .or_insert_with(|| Thread {
                    id: incoming.id.clone(),
                    ..Default::default()
                });
            if let Some(seed) = &seed {
                let mut unread = seed.unread.clone();
                unread.fresh = true;
                entry.unread.merge_metadata(&unread);
            }
            crate::chat::merge_thread_into(entry, incoming, true);
            entry.unread.observe(&entry.messages);
            if let Some(seed) = seed {
                entry
                    .unread
                    .unread_ids
                    .retain(|id| !seed.read_ids.contains(id));
            }
            if let Ok(mut counts) = self.counts.lock() {
                counts.insert(entry.id.clone(), entry.unread.count());
            }
            let skip = entry
                .messages
                .len()
                .saturating_sub(crate::store::MessageStore::PRELOAD_MSGS);
            entry.messages.drain(..skip);
        }
        if state.0.is_empty() || state.1 {
            return false;
        }
        state.1 = true;
        true
    }
    pub(crate) fn seed(&self, threads: &[Thread]) {
        if let Ok(mut counts) = self.counts.lock() {
            for t in threads {
                counts.insert(t.id.clone(), t.unread.count());
            }
        }
        if let Ok(mut names) = self.names.lock() {
            for t in threads {
                names.insert(t.id.clone(), (t.name.clone(), t.avatar.clone()));
            }
        }
        {
            let mut alerts = self.alerts.lock().unwrap_or_else(|e| e.into_inner());
            for thread in threads {
                let latest = thread
                    .messages
                    .iter()
                    .filter_map(|m| m.id.parse::<u128>().ok())
                    .max()
                    .unwrap_or(0);
                let cursor = alerts.0.entry(thread.id.clone()).or_default();
                *cursor = (*cursor).max(latest);
            }
        }
        let baselines: Vec<_> = threads
            .iter()
            .map(|thread| {
                let read_ids = thread
                    .messages
                    .iter()
                    .rev()
                    .take(crate::store::MessageStore::PRELOAD_MSGS)
                    .filter(|message| !thread.unread.unread_ids.contains(&message.id))
                    .map(|message| message.id.clone())
                    .collect();
                (
                    thread.id.clone(),
                    Baseline {
                        read_ids,
                        unread: thread.unread.clone(),
                    },
                )
            })
            .collect();
        self.baselines
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(baselines);
    }
    #[cfg(any(windows, test))]
    pub(crate) fn take_notice(&self) -> Option<crate::desktop_notice::Notice> {
        self.notice.lock().ok().and_then(|mut s| s.take())
    }
    #[cfg(any(windows, test))]
    pub(crate) fn total(&self) -> usize {
        self.counts
            .lock()
            .map(|s| s.values().copied().sum())
            .unwrap_or(0)
    }
    pub(crate) fn set_muted(&self, values: &HashMap<String, crate::store::ConversationPreference>) {
        *self.muted.lock().unwrap_or_else(|e| e.into_inner()) = values
            .iter()
            .filter(|(_, value)| value.muted)
            .map(|(id, _)| id.clone())
            .collect();
    }
    #[cfg(any(windows, test))]
    pub(crate) fn take_notification(&self) -> bool {
        let mut alerts = self.alerts.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut alerts.1)
    }
    pub(crate) fn take(&self) -> Vec<Thread> {
        let mut state = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        state.1 = false;
        std::mem::take(&mut state.0).into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::ChatMessage;

    #[test]
    fn hidden_delivery_coalesces_wakes_and_keeps_unread_beyond_resident_messages() {
        let queue = LiveUpdates::default();
        let mut wakes = 0;
        for n in 1..=1000 {
            wakes += usize::from(queue.publish(vec![Thread {
                id: "456".into(),
                messages: vec![ChatMessage {
                    id: n.to_string(),
                    ts: n,
                    text: "fixture".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }]));
        }
        assert_eq!(wakes, 1);
        let batch = queue.take();
        assert_eq!(batch.len(), 1);
        assert_eq!(
            batch[0].messages.len(),
            crate::store::MessageStore::PRELOAD_MSGS
        );
        assert_eq!(batch[0].unread.count(), 1000);
        assert!(queue.take().is_empty());
        assert!(queue.publish(vec![Thread {
            id: "456".into(),
            ..Default::default()
        }]));
    }

    #[test]
    fn coalesced_read_cursor_prunes_earlier_unread_and_preserves_silent_policy() {
        let queue = LiveUpdates::default();
        for n in 1..=5 {
            queue.publish(vec![Thread {
                id: "456".into(),
                messages: vec![ChatMessage {
                    id: n.to_string(),
                    ts: n,
                    ..Default::default()
                }],
                ..Default::default()
            }]);
        }
        let mut update = Thread {
            id: "456".into(),
            ..Default::default()
        };
        update.unread.advance_seen("4");
        update.unread.fresh = true;
        queue.publish(vec![update]);
        let batch = queue.take();
        assert_eq!(batch[0].unread.count(), 1);
        assert!(batch[0].unread.unread_ids.contains("5"));
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    #[test]
    fn later_silent_policy_removes_an_already_queued_unread_id() {
        let queue = LiveUpdates::default();
        let mut threads = vec![Thread {
            id: "456".into(),
            ..Default::default()
        }];
        for policy in [1, 0] {
            let raw = serde_json::json!({"data":{"msgs":[{"msgId":"11","uidFrom":"456","idTo":"0","content":"fixture","paramsExt":{"countUnread":policy}}]}});
            crate::chat::merge_previews(&mut threads, &raw);
            queue.publish(threads.clone());
        }
        assert_eq!(queue.take()[0].unread.count(), 0);
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use crate::chat::ChatMessage;
    #[test]
    #[ignore = "manual synthetic live-delivery queue measurement"]
    fn measure_live_delivery_coalescing() {
        let mut direct = Vec::new();
        let mut coalesced = Vec::new();
        for _ in 0..5 {
            let updates: Vec<_> = (1..=1000)
                .map(|n| Thread {
                    id: "fixture-thread".into(),
                    messages: vec![ChatMessage {
                        id: n.to_string(),
                        ts: n,
                        text: "Synthetic message".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect();
            let start = std::time::Instant::now();
            let mut queue = std::collections::VecDeque::new();
            for update in &updates {
                queue.push_back(update.clone());
            }
            std::hint::black_box(&queue);
            direct.push(start.elapsed().as_secs_f64() * 1000.0);
            let queue = LiveUpdates::default();
            let start = std::time::Instant::now();
            for update in updates {
                queue.publish(vec![update]);
            }
            let batch = queue.take();
            coalesced.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(batch[0].messages.len(), 20);
            assert_eq!(batch[0].unread.count(), 1000);
        }
        for (task, mut times, events, messages) in [
            ("direct_event_queue", direct, 1000, 1000),
            ("coalesced_live_queue", coalesced, 1, 20),
        ] {
            times.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::json!({"task":task,"iterations":5,"dataset_messages":1000,"threads":1,
                "min_ms":times[0],"avg_ms":times.iter().sum::<f64>()/5.0,"p50_ms":times[2],"p95_ms":times[4],"p99_ms":times[4],
                "queued_events":events,"resident_messages":messages,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"profile":"debug"})
            );
        }
    }
}

#[cfg(test)]
mod baseline_tests {
    use super::*;
    #[test]
    fn cached_or_locally_read_replays_cannot_trigger_background_attention() {
        let queue = LiveUpdates::default();
        let thread = Thread {
            id: "456".into(),
            messages: vec![crate::chat::ChatMessage {
                id: "11".into(),
                ts: 1700000000000,
                ..Default::default()
            }],
            ..Default::default()
        };
        queue.seed(std::slice::from_ref(&thread));
        queue.publish(vec![thread.clone()]);
        assert_eq!(queue.take()[0].unread.count(), 0);
        queue.publish(vec![Thread {
            messages: vec![crate::chat::ChatMessage {
                id: "12".into(),
                ts: 1700000000001,
                ..Default::default()
            }],
            ..thread.clone()
        }]);
        assert_eq!(queue.take()[0].unread.count(), 1);
        let mut read = thread;
        read.unread.advance_seen("12");
        queue.seed(&[read]);
        queue.publish(vec![Thread {
            id: "456".into(),
            messages: vec![crate::chat::ChatMessage {
                id: "12".into(),
                ..Default::default()
            }],
            ..Default::default()
        }]);
        assert_eq!(queue.take()[0].unread.count(), 0);
    }
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    use crate::chat::ChatMessage;
    fn thread(id: &str, outgoing: bool) -> Thread {
        Thread {
            id: "conversation".into(),
            messages: vec![ChatMessage {
                id: id.into(),
                is_outgoing: outgoing,
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn popup_deduplicates_replay_even_after_gui_drains_mailbox() {
        let updates = LiveUpdates::default();
        updates.seed(&[thread("100", false)]);
        updates.publish(vec![thread("100", false)]);
        assert!(!updates.take_notification());
        updates.publish(vec![thread("101", false)]);
        assert!(updates.take_notification());
        updates.take();
        updates.publish(vec![thread("101", false)]);
        assert!(!updates.take_notification());
        updates.publish(vec![thread("102", true)]);
        assert!(!updates.take_notification());
        updates.publish(vec![thread("103", false)]);
        assert!(updates.take_notification());
    }
    #[test]
    fn muted_conversation_suppresses_popup() {
        let updates = LiveUpdates::default();
        updates.set_muted(&HashMap::from([(
            "conversation".into(),
            crate::store::ConversationPreference {
                muted: true,
                ..Default::default()
            },
        )]));
        updates.publish(vec![thread("101", false)]);
        assert!(!updates.take_notification());
    }
}

#[cfg(test)]
mod notification_contract_tests {
    use super::*;
    #[test]
    fn group_notification_and_hidden_count_use_live_message_identity() {
        let q = LiveUpdates::default();
        q.seed(&[Thread {
            id: "group".into(),
            name: "Nhóm dự án".into(),
            is_group: true,
            ..Default::default()
        }]);
        let message = crate::chat::ChatMessage {
            id: "12".into(),
            sender: "An".into(),
            text: "Xin chào".into(),
            ts: 100,
            ..Default::default()
        };
        q.publish(vec![Thread {
            id: "group".into(),
            is_group: true,
            messages: vec![message.clone()],
            ..Default::default()
        }]);
        let notice = q.take_notice().unwrap();
        assert_eq!(notice.title, "Nhóm dự án");
        assert_eq!(notice.body, "An: Xin chào");
        assert_eq!(q.total(), 1);
        q.take();
        q.publish(vec![Thread {
            id: "group".into(),
            is_group: true,
            messages: vec![message],
            ..Default::default()
        }]);
        assert!(q.take_notice().is_none());
    }
}
