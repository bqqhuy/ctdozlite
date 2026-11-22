//! Opt-in bounded performance counters with fixed labels and no account or payload fields.
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const SAMPLE_LIMIT: usize = 512;

/// Fixed units: durations are milliseconds, queue depth is an item count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Metric {
    GuiFrameWork,
    SnapshotClone,
    SnapshotWrite,
    OutgoingWrite,
    ReceivedWrite,
    QueueWait,
    HttpRequest,
    BodyAck,
    SendTotal,
    IngressPage,
    IngressQueueDepth,
    SqliteBusy,
    SqliteFailure,
    DispatchToFirstOutcome,
    DispatchToTerminal,
}

#[derive(Default)]
struct Series {
    count: u64,
    samples: VecDeque<f64>,
}
impl Series {
    fn record(&mut self, value: f64) {
        self.count += 1;
        if self.samples.len() == SAMPLE_LIMIT {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }
}

fn state() -> &'static Mutex<BTreeMap<Metric, Series>> {
    static STATE: OnceLock<Mutex<BTreeMap<Metric, Series>>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}

/// Enable only on a requested test run; normal application runs do not collect these samples.
pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("CTDOZ_PERF").is_ok_and(|value| value == "1"))
}

/// Record a numeric allowlisted observation without performing I/O.
pub(crate) fn sample(metric: Metric, value: f64) {
    if !enabled() || !value.is_finite() || value < 0.0 {
        return;
    }
    state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(metric)
        .or_default()
        .record(value);
}

fn dispatches() -> &'static Mutex<BTreeMap<String, (Instant, bool)>> {
    static DISPATCHES: OnceLock<Mutex<BTreeMap<String, (Instant, bool)>>> = OnceLock::new();
    DISPATCHES.get_or_init(Default::default)
}

/// Track dispatch in memory by a process-keyed tag; the tag is never serialized.
pub(crate) fn dispatched(client_id: &str) {
    if !enabled() {
        return;
    }
    let Some(tag) = crate::service_resolver::intent_tag(client_id) else {
        return;
    };
    let mut dispatches = dispatches().lock().unwrap_or_else(|e| e.into_inner());
    if dispatches.len() >= SAMPLE_LIMIT {
        if let Some(oldest) = dispatches
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(id, _)| id.clone())
        {
            dispatches.remove(&oldest);
        }
    }
    dispatches.insert(tag, (Instant::now(), false));
}

/// Observe application of the first visible outcome, not GPU presentation or server delivery.
pub(crate) fn observe_outgoing(client_id: &str, terminal: bool) {
    if !enabled() {
        return;
    }
    let Some(tag) = crate::service_resolver::intent_tag(client_id) else {
        return;
    };
    let mut dispatches = dispatches().lock().unwrap_or_else(|e| e.into_inner());
    let Some((at, seen)) = dispatches.get_mut(&tag) else {
        return;
    };
    let elapsed = at.elapsed().as_secs_f64() * 1000.0;
    let first = !*seen;
    *seen = true;
    if terminal {
        dispatches.remove(&tag);
    }
    drop(dispatches);
    if first {
        sample(Metric::DispatchToFirstOutcome, elapsed);
    }
    if terminal {
        sample(Metric::DispatchToTerminal, elapsed);
    }
}

/// Time one local stage; this measures work duration, not the display's presentation interval.
pub(crate) struct Scope {
    metric: Metric,
    started: Option<Instant>,
}
impl Scope {
    pub(crate) fn new(metric: Metric) -> Self {
        Self {
            metric,
            started: enabled().then(Instant::now),
        }
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            sample(self.metric, started.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

/// Aggregate over the latest bounded window; count includes evicted observations.
#[derive(Serialize)]
pub(crate) struct Summary {
    metric: Metric,
    count: u64,
    sample_count: usize,
    min: f64,
    avg: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
}
fn summarize(metric: Metric, count: u64, mut values: Vec<f64>) -> Option<Summary> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let percentile = |p: usize| values[(values.len() * p).div_ceil(100).saturating_sub(1)];
    Some(Summary {
        metric,
        count,
        sample_count: values.len(),
        min: values[0],
        avg: values.iter().sum::<f64>() / values.len() as f64,
        p50: percentile(50),
        p95: percentile(95),
        p99: percentile(99),
        max: values[values.len() - 1],
    })
}

/// Snapshot numerical summaries only; no arbitrary strings can reach the serialized result.
pub(crate) fn snapshot() -> Vec<Summary> {
    let copy: Vec<_> = state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|(metric, series)| {
            (
                *metric,
                series.count,
                series.samples.iter().copied().collect(),
            )
        })
        .collect();
    copy.into_iter()
        .filter_map(|(metric, count, samples)| summarize(metric, count, samples))
        .collect()
}

/// Record only SQLite result codes, never SQL, paths, or server/user text.
pub(crate) fn database_error(error: &anyhow::Error) {
    let code = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<rusqlite::Error>())
        .and_then(rusqlite::Error::sqlite_error_code);
    if matches!(
        code,
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ) {
        sample(Metric::SqliteBusy, 1.0);
    }
    sample(Metric::SqliteFailure, 1.0);
    crate::logger::log_error(
        "account persistence",
        &format!("SQLite result category: {code:?}"),
    );
}

/// Periodic exporter runs on the worker; the diagnostics writer owns all filesystem work.
pub(crate) async fn export_loop() {
    if !enabled() {
        return;
    }
    let mut timer = tokio::time::interval(Duration::from_secs(5));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        crate::transfer_diagnostics::runtime_performance(snapshot());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_window_retains_tail_and_reports_real_percentiles() {
        let mut series = Series::default();
        for value in 0..1024 {
            series.record(value as f64);
        }
        let summary =
            summarize(Metric::SnapshotWrite, series.count, series.samples.into()).unwrap();
        assert_eq!(summary.count, 1024);
        assert_eq!(summary.sample_count, SAMPLE_LIMIT);
        assert_eq!(summary.min, 512.0);
        assert_eq!(summary.p50, 767.0);
        assert_eq!(summary.p95, 998.0);
        assert_eq!(summary.max, 1023.0);
    }
    #[test]
    fn serialized_summary_contains_only_fixed_metric_and_numbers() {
        let summary = summarize(Metric::BodyAck, 3, vec![1.0, 10.0, 100.0]).unwrap();
        let value = serde_json::to_value(summary).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object["metric"], "body_ack");
        assert!(object
            .iter()
            .all(|(key, value)| key == "metric" || value.is_number()));
    }
}
