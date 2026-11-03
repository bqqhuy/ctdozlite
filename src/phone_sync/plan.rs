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
