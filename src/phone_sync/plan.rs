//! Recent-first request ranges and conservative coverage-based phone-sync resumption.
use super::import::number;
use crate::store::PhoneSyncRange;
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
const DAY: i64 = 86_400_000;
const OVERLAP: i64 = 120_000;

#[derive(Clone, Debug)]
pub(super) struct Conversation {
    pub partition: String,
    pub activity: i64,
}
#[derive(Clone, Debug)]
pub(super) struct Query {
    pub partition: String,
    pub from: i64,
    pub to: i64,
}

pub(super) fn conversations(values: &[Value]) -> Result<Vec<Conversation>> {
    let mut unique: HashMap<String, i64> = HashMap::new();
    for c in values {
        let id = c
            .get("convId")
            .and_then(Value::as_str)
            .context("Missing phone conversation ID")?;
        ensure!(
            !id.is_empty() && id.len() <= 512 && !id.contains('/'),
            "Invalid phone conversation ID"
        );
        let prefix = if c.get("convType").and_then(Value::as_u64) == Some(2) {
            "group"
        } else {
            "oneone"
        };
        let activity = crate::clock::normalize_ts_ms(number(c, "lastTs").parse().unwrap_or(0));
        unique
            .entry(format!("{prefix}/{id}"))
            .and_modify(|ts| *ts = (*ts).max(activity))
            .or_insert(activity);
    }
    let mut out: Vec<_> = unique
        .into_iter()
        .map(|(partition, activity)| Conversation {
            partition,
            activity,
        })
        .collect();
    out.sort_by(|a, b| {
        b.activity
            .cmp(&a.activity)
            .then_with(|| a.partition.cmp(&b.partition))
    });
    Ok(out)
}

pub(super) fn queries(
    conversations: &[Conversation],
    coverage: &HashMap<String, PhoneSyncRange>,
    now: i64,
    recent_only: bool,
    floor: i64,
) -> Vec<Query> {
    conversations
        .iter()
        .filter_map(|c| {
            let saved = coverage.get(&c.partition);
            let from = if recent_only {
                saved
                    .map(|s| s.to_ts.saturating_sub(OVERLAP).max(s.from_ts))
                    .unwrap_or(now.saturating_sub(7 * DAY))
                    .max(now.saturating_sub(7 * DAY))
                    .max(0)
            } else if let Some(saved) = saved.filter(|s| s.from_ts <= floor) {
                // Coverage already reaches the requested floor: fetch only the delta.
                saved.to_ts.saturating_sub(OVERLAP).max(floor).min(now)
            } else {
                floor
            };
            (now >= from).then(|| Query {
                partition: c.partition.clone(),
                from,
                to: now,
            })
        })
        .collect()
}

/// How long an unrecognised conversation waits before it is requested again with an
/// unchanged `lastTs`. Its messages were already quarantined in `phone_unmapped`.
pub(super) const DEFER_UNMAPPED: i64 = 3 * DAY;
/// Largest accepted file for deferred-partition state.
const DEFERRED_LIMIT: usize = 4 * 1024 * 1024;

/// A conversation the client could not map to a local thread on its last completed request.
/// This is a postponement only, never coverage.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DeferredEntry {
    pub last_ts: i64,
    pub until: i64,
}
/// Per-account deferred state; a missing or damaged file means "nothing deferred".
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Deferred(pub HashMap<String, DeferredEntry>);
impl Deferred {
    pub fn load(path: &std::path::Path) -> Self {
        crate::limits::read_file(path, DEFERRED_LIMIT)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        crate::storage::atomic_write(path, &serde_json::to_vec(self)?)
    }
    fn holds(&self, partition: &str, activity: i64, now: i64) -> bool {
        self.0
            .get(partition)
            .is_some_and(|d| d.last_ts == activity && d.until > now)
    }
    /// Postpone `partition` until its `lastTs` changes or `DEFER_UNMAPPED` passes.
    pub fn postpone(&mut self, partition: &str, activity: i64, now: i64) {
        self.0.insert(
            partition.to_owned(),
            DeferredEntry {
                last_ts: activity,
                until: now.saturating_add(DEFER_UNMAPPED),
            },
        );
    }
    /// Drop expired entries and partitions that are now recognised.
    pub fn prune(&mut self, owners: &HashMap<String, String>, now: i64) {
        self.0
            .retain(|p, d| d.until > now && !owners.contains_key(p));
    }
}

/// Requests for one run plus what was left out and why.
#[derive(Debug, Default)]
pub(super) struct Planned {
    pub queries: Vec<Query>,
    /// Coverage already reaches the conversation's `lastTs` (or it has nothing in range).
    pub unchanged: usize,
    /// Unrecognised conversations postponed with an unchanged `lastTs`.
    pub deferred: usize,
}

/// Inputs that decide which conversations an update run may leave out.
pub(super) struct Update<'a> {
    pub coverage: &'a HashMap<String, PhoneSyncRange>,
    pub owners: &'a HashMap<String, String>,
    pub deferred: &'a Deferred,
    pub now: i64,
    pub floor: i64,
    pub full_rescan: bool,
}

/// `lastTs` is trusted only when present and not implausibly in the future.
fn trusted_activity(activity: i64, now: i64) -> bool {
    activity > 0 && activity <= now.saturating_add(DAY)
}

impl Update<'_> {
    /// Like Zalo PC's `noNeedUpdatePreviewConversations`, skip conversations whose stored
    /// coverage already reaches the requested floor and their last activity, and stop each
    /// request at that activity. Anything uncertain falls back to the plain request.
    pub fn plan(&self, conversations: &[Conversation], recent_only: bool) -> Planned {
        let mut planned = Planned::default();
        let base = queries(
            conversations,
            self.coverage,
            self.now,
            recent_only,
            self.floor,
        );
        let activity: HashMap<&str, i64> = conversations
            .iter()
            .map(|c| (c.partition.as_str(), c.activity))
            .collect();
        for mut query in base {
            let last = activity.get(query.partition.as_str()).copied().unwrap_or(0);
            if self.full_rescan {
                planned.queries.push(query);
                continue;
            }
            if !self.owners.contains_key(&query.partition) {
                if self.deferred.holds(&query.partition, last, self.now) {
                    planned.deferred += 1;
                } else {
                    planned.queries.push(query);
                }
                continue;
            }
            if recent_only || !trusted_activity(last, self.now) {
                planned.queries.push(query);
                continue;
            }
            let covered = self
                .coverage
                .get(&query.partition)
                .is_some_and(|saved| saved.from_ts <= self.floor && saved.to_ts >= last);
            if covered || last < query.from.max(self.floor) {
                planned.unchanged += 1;
                continue;
            }
            query.to = query.to.min(last.saturating_add(OVERLAP));
            planned.queries.push(query);
        }
        planned
    }
}

pub(super) fn extend(
    coverage: &mut HashMap<String, PhoneSyncRange>,
    queries: &[Query],
    owners: &HashMap<String, String>,
) -> Result<()> {
    for query in queries {
        let owner = owners.get(&query.partition).cloned().unwrap_or_default();
        if let Some(saved) = coverage.get_mut(&query.partition) {
            ensure!(
                query.from <= saved.to_ts && query.to >= saved.from_ts,
                "Disconnected phone history range"
            );
            saved.from_ts = saved.from_ts.min(query.from);
            saved.to_ts = saved.to_ts.max(query.to);
        } else {
            coverage.insert(
                query.partition.clone(),
                PhoneSyncRange {
                    thread_id: owner,
                    from_ts: query.from,
                    to_ts: query.to,
                },
            );
        }
    }
    Ok(())
}
/// Merge authenticated completed ranges only when they touch durable coverage.
/// Disconnected recent ranges wait until a later phase fills the gap; a crash
/// may replay those ranges but can never mark the intervening history complete.
pub(super) fn checkpoint_completed(
    coverage: &mut HashMap<String, PhoneSyncRange>,
    pending: &mut Vec<Query>,
    owners: &HashMap<String, String>,
) -> Result<()> {
    while let Some(index) = pending.iter().position(|q| {
        coverage
            .get(&q.partition)
            .is_none_or(|saved| q.from <= saved.to_ts && q.to >= saved.from_ts)
    }) {
        let query = pending.remove(index);
        extend(coverage, &[query], owners)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_transfer_is_one_window_and_resume_is_delta_only_after_complete_coverage() {
        let now = 1_800_000_000_000;
        let c = vec![Conversation {
            partition: "group/test".into(),
            activity: now - 100 * DAY,
        }];
        let mut saved = HashMap::new();
        assert_eq!(queries(&c, &saved, now, false, 0)[0].from, 0);
        let owner = HashMap::from([("group/test".into(), "test".into())]);
        extend(
            &mut saved,
            &queries(&c, &HashMap::new(), now, false, 0),
            &owner,
        )
        .unwrap();
        assert_eq!(
            queries(&c, &saved, now + DAY, false, 0)[0].from,
            now - OVERLAP
        );
        assert_eq!(
            queries(&c, &HashMap::new(), now, true, 0)[0].from,
            now - 7 * DAY
        );
    }
    #[test]
    fn disconnected_completed_ranges_do_not_claim_missing_history() {
        let owners = HashMap::new();
        let mut saved = HashMap::from([(
            "p".into(),
            PhoneSyncRange {
                thread_id: String::new(),
                from_ts: 0,
                to_ts: 10,
            },
        )]);
        let mut pending = vec![Query {
            partition: "p".into(),
            from: 20,
            to: 30,
        }];
        checkpoint_completed(&mut saved, &mut pending, &owners).unwrap();
        assert_eq!(saved["p"].to_ts, 10);
        assert_eq!(pending.len(), 1);
        pending.push(Query {
            partition: "p".into(),
            from: 10,
            to: 20,
        });
        checkpoint_completed(&mut saved, &mut pending, &owners).unwrap();
        assert_eq!(saved["p"].to_ts, 30);
        assert!(pending.is_empty());
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;
    const NOW: i64 = 1_800_000_000_000;
    fn conv(partition: &str, activity: i64) -> Conversation {
        Conversation {
            partition: partition.into(),
            activity,
        }
    }
    fn range(from_ts: i64, to_ts: i64) -> PhoneSyncRange {
        PhoneSyncRange {
            thread_id: "t".into(),
            from_ts,
            to_ts,
        }
    }
    fn owners(ids: &[&str]) -> HashMap<String, String> {
        ids.iter().map(|p| (p.to_string(), "t".into())).collect()
    }
    #[test]
    fn covered_conversations_are_skipped_and_others_stop_at_last_activity() {
        let coverage = HashMap::from([
            ("group/old".to_string(), range(0, NOW - DAY)),
            ("group/new".to_string(), range(0, NOW - 3 * DAY)),
        ]);
        let owners = owners(&["group/old", "group/new", "group/fresh"]);
        let deferred = Deferred::default();
        let update = Update {
            coverage: &coverage,
            owners: &owners,
            deferred: &deferred,
            now: NOW,
            floor: 0,
            full_rescan: false,
        };
        let planned = update.plan(
            &[
                conv("group/old", NOW - 2 * DAY),
                conv("group/new", NOW - DAY),
                conv("group/fresh", NOW - 10 * DAY),
            ],
            false,
        );
        assert_eq!(planned.unchanged, 1);
        let new = planned
            .queries
            .iter()
            .find(|q| q.partition == "group/new")
            .unwrap();
        assert_eq!(new.from, NOW - 3 * DAY - OVERLAP);
        assert_eq!(new.to, NOW - DAY + OVERLAP);
        let fresh = planned
            .queries
            .iter()
            .find(|q| q.partition == "group/fresh")
            .unwrap();
        assert_eq!((fresh.from, fresh.to), (0, NOW - 10 * DAY + OVERLAP));
    }
    #[test]
    fn untrusted_activity_and_partial_coverage_fall_back_to_full_request() {
        let coverage = HashMap::from([
            ("group/a".to_string(), range(0, NOW - DAY)),
            ("group/b".to_string(), range(NOW - 30 * DAY, NOW - DAY)),
        ]);
        let owners = owners(&["group/a", "group/b"]);
        let deferred = Deferred::default();
        let update = Update {
            coverage: &coverage,
            owners: &owners,
            deferred: &deferred,
            now: NOW,
            floor: 0,
            full_rescan: false,
        };
        for bad in [0, -5, NOW + 2 * DAY] {
            let planned = update.plan(&[conv("group/a", bad)], false);
            assert_eq!(planned.queries.len(), 1, "activity {bad}");
            assert_eq!(planned.queries[0].to, NOW);
        }
        // Coverage that does not reach the floor is never treated as complete.
        let planned = update.plan(&[conv("group/b", NOW - 2 * DAY)], false);
        assert_eq!(planned.unchanged, 0);
        assert_eq!(planned.queries[0].from, 0);
    }
    #[test]
    fn activity_older_than_floor_has_nothing_to_fetch() {
        let coverage = HashMap::new();
        let owners = owners(&["group/a"]);
        let deferred = Deferred::default();
        let update = Update {
            coverage: &coverage,
            owners: &owners,
            deferred: &deferred,
            now: NOW,
            floor: NOW - 30 * DAY,
            full_rescan: false,
        };
        let planned = update.plan(&[conv("group/a", NOW - 60 * DAY)], false);
        assert!(planned.queries.is_empty());
        assert_eq!(planned.unchanged, 1);
    }
    #[test]
    fn unmapped_conversations_are_deferred_until_activity_changes_or_expiry() {
        let coverage = HashMap::new();
        let owners = HashMap::new();
        let mut deferred = Deferred::default();
        deferred.postpone("oneone/x", NOW - DAY, NOW);
        let plan = |deferred: &Deferred, now: i64, activity: i64, full_rescan: bool| {
            Update {
                coverage: &coverage,
                owners: &owners,
                deferred,
                now,
                floor: 0,
                full_rescan,
            }
            .plan(&[conv("oneone/x", activity)], false)
        };
        let held = plan(&deferred, NOW + DAY, NOW - DAY, false);
        assert_eq!((held.queries.len(), held.deferred), (0, 1));
        assert_eq!(plan(&deferred, NOW + DAY, NOW, false).queries.len(), 1);
        assert_eq!(
            plan(&deferred, NOW + DEFER_UNMAPPED, NOW - DAY, false)
                .queries
                .len(),
            1
        );
        assert_eq!(plan(&deferred, NOW + DAY, NOW - DAY, true).queries.len(), 1);
        // A partition that becomes recognised leaves the deferred state.
        deferred.prune(
            &HashMap::from([("oneone/x".to_string(), "x".to_string())]),
            NOW,
        );
        assert!(deferred.0.is_empty());
    }
    #[test]
    fn damaged_deferred_file_reads_as_empty_and_saves_atomically() {
        let root = std::env::temp_dir().join(format!(
            "deferred-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("a.sync-deferred.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert!(Deferred::load(&path).0.is_empty());
        let mut deferred = Deferred::default();
        deferred.postpone("group/g", 5, NOW);
        deferred.save(&path).unwrap();
        assert_eq!(Deferred::load(&path).0, deferred.0);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod floor_tests {
    use super::*;
    #[test]
    fn floor_limits_new_ranges_and_delta_applies_once_coverage_reaches_floor() {
        let now = 1_800_000_000_000;
        let floor = now - 30 * DAY;
        let c = vec![Conversation {
            partition: "oneone/a".into(),
            activity: now,
        }];
        assert_eq!(
            queries(&c, &HashMap::new(), now, false, floor)[0].from,
            floor
        );
        let reaching = HashMap::from([(
            "oneone/a".to_string(),
            PhoneSyncRange {
                thread_id: "1".into(),
                from_ts: floor - DAY,
                to_ts: now - DAY,
            },
        )]);
        assert_eq!(
            queries(&c, &reaching, now, false, floor)[0].from,
            now - DAY - OVERLAP
        );
        let short = HashMap::from([(
            "oneone/a".to_string(),
            PhoneSyncRange {
                thread_id: "1".into(),
                from_ts: floor + DAY,
                to_ts: now - DAY,
            },
        )]);
        assert_eq!(queries(&c, &short, now, false, floor)[0].from, floor);
    }
}
