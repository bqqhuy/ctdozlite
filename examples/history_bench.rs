//! Offline measurement of the history merge hot path; uses synthetic data only.
use std::time::Instant;
use zalo_rust::chat::{merge_thread_into, ChatMessage, Thread};
fn thread(start: usize, count: usize) -> Thread {
    Thread {
        presentation: Default::default(),
        unread: Default::default(),
        id: "thread-1".into(),
        name: "Benchmark".into(),
        preview: String::new(),
        time: String::new(),
        avatar: None,
        is_group: true,
        messages: (start..start + count)
            .map(|i| ChatMessage {
                id: format!("msg-{i}"),
                text: "Repeated real message".into(),
                ts: 1000 + i as i64,
                time: "10:00".into(),
                ..Default::default()
            })
            .collect(),
    }
}
fn main() {
    let base = thread(0, 1000);
    let incoming = thread(500, 1000);
    let mut samples = Vec::new();
    for _ in 0..20 {
        let mut target = base.clone();
        let delta = incoming.clone();
        let now = Instant::now();
        merge_thread_into(&mut target, delta, true);
        samples.push(now.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(target.messages.len(), 1500);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({
            "task":"merge_1000_into_1000_500_overlap_repeated_text",
            "iterations":samples.len(),"min_ms":samples[0],
            "avg_ms":samples.iter().sum::<f64>()/samples.len() as f64,
            "p50_ms":samples[10],"p95_ms":samples[18],"p99_ms":samples[19],
            "arch":std::env::consts::ARCH,"os":std::env::consts::OS,"profile":"debug"
        })
    );
}
