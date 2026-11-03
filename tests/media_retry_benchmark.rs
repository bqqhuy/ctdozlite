//! Controlled queue pressure measurement; does not measure Zalo CDN latency.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use zalo_rust::media_recovery::{Failure, Retry};

#[tokio::test]
#[ignore = "controlled synthetic queue benchmark; run explicitly with --ignored --nocapture"]
async fn known_terminal_failures_leave_slots_for_visible_images() {
    async fn trial(suppress: bool) -> f64 {
        let slots = Arc::new(tokio::sync::Semaphore::new(4));
        let mut work = tokio::task::JoinSet::new();
        let started = Instant::now();
        for _ in 0..50 {
            let failure = Retry::next(Failure::Recovery, None);
            if suppress && failure.at.is_none() {
                continue;
            }
            let slots = slots.clone();
            work.spawn(async move {
                let _slot = slots.acquire().await.unwrap();
                tokio::time::sleep(Duration::from_millis(5)).await;
            });
        }
        // Let admitted retries queue before the user's newly visible image.
        tokio::task::yield_now().await;
        let _slot = slots.acquire().await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        drop(_slot);
        while work.join_next().await.is_some() {}
        elapsed
    }
    for (name, suppress) in [
        ("legacy_unlimited_terminal_retry", false),
        ("finite_classified_retry", true),
    ] {
        let mut values = Vec::new();
        for _ in 0..10 {
            values.push(trial(suppress).await);
        }
        values.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"task":name,"iterations":values.len(),"min_ms":values[0],"avg_ms":values.iter().sum::<f64>()/values.len() as f64,"p50_ms":values[4],"p95_ms":values[9],"p99_ms":values[9],"dataset":"50 known terminal failures + 1 visible image; 4 slots, synthetic 5ms/job","os":std::env::consts::OS,"arch":std::env::consts::ARCH})
        );
    }
}
