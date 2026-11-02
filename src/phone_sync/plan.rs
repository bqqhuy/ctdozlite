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
    phase: usize,
) -> Vec<Query> {
    conversations
        .iter()
        .filter_map(|c| {
            let anchor = if c.activity > 0 {
                c.activity.min(now)
            } else {
                now
            };
            let saved = coverage.get(&c.partition);
            let (from, to) = match phase {
                0 => {
                    if let Some(saved) = saved {
                        (
                            saved
                                .to_ts
                                .saturating_sub(OVERLAP)
                                .max(saved.from_ts)
                                .min(now),
                            now,
                        )
                    } else {
                        (anchor.saturating_sub(7 * DAY).max(0), now)
                    }
                }
                1 | 2 => {
                    let saved = saved?;
                    let target = if phase == 1 {
                        anchor.saturating_sub(90 * DAY).max(0)
                    } else {
                        0
                    };
                    if saved.from_ts <= target {
                        return None;
                    }
                    (target, saved.from_ts.saturating_add(OVERLAP).min(now))
                }
                _ => return None,
            };
            (to >= from).then(|| Query {
                partition: c.partition.clone(),
                from,
                to,
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
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn activity_orders_groups_and_direct_before_older_partitions() {
        let c = conversations(&[
            json!({"convId":"a","lastTs":"1700000000000"}),
            json!({"convId":"z","convType":2,"lastTs":"1700000300000"}),
            json!({"convId":"a","lastTs":"1690000000000"}),
        ])
        .unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].partition, "group/z");
    }
    #[test]
    fn recent_then_backfill_resume_without_ever_claiming_a_missing_interval() {
        let now = 1_800_000_000_000;
        let c = vec![Conversation {
            partition: "oneone/fake".into(),
            activity: now,
        }];
        let owners = HashMap::from([("oneone/fake".into(), "123".into())]);
        let mut saved = HashMap::new();
        let recent = queries(&c, &saved, now, 0);
        assert_eq!(recent[0].from, now - 7 * DAY);
        extend(&mut saved, &recent, &owners).unwrap();
        let retry = queries(&c, &saved, now + DAY, 0);
        assert!(retry[0].from > now - 3 * 60_000);
        let old = queries(&c, &saved, now, 1);
        assert_eq!(old[0].from, now - 90 * DAY);
        extend(&mut saved, &old, &owners).unwrap();
        assert!(queries(&c, &saved, now, 1).is_empty());
        let oldest = queries(&c, &saved, now, 2);
        assert_eq!(oldest[0].from, 0);
        extend(&mut saved, &oldest, &owners).unwrap();
        assert!(queries(&c, &saved, now, 2).is_empty());
        assert!(queries(&c, &saved, now + DAY, 0)[0].from > 0);
        assert!(extend(
            &mut saved,
            &[Query {
                partition: "oneone/fake".into(),
                from: now + DAY,
                to: now + 2 * DAY
            }],
            &owners
        )
        .is_err());
    }
    #[test]
    fn inactive_conversation_recent_window_is_anchored_at_its_last_activity() {
        let now = 1_800_000_000_000;
        let c = vec![Conversation {
            partition: "group/fake".into(),
            activity: now - 1000 * DAY,
        }];
        let q = queries(&c, &HashMap::new(), now, 0);
        assert_eq!(q[0].from, now - 1007 * DAY);
        assert_eq!(q[0].to, now);
    }
}
