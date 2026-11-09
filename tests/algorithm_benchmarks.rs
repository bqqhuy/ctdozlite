//! Opt-in local comparison of sequential upload checkpoint algorithms.
use std::time::Instant;
use zalo_rust::upload::ResumableUploadState;

fn measure(name: &str, mut task: impl FnMut()) {
    let mut times = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        task();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{name},7,{:.3},{:.3},{:.3},{:.3},{:.3}",
        times[0],
        times.iter().sum::<f64>() / 7.0,
        times[3],
        times[6],
        times[6]
    );
}

#[test]
#[ignore = "controlled algorithm benchmark; run explicitly with --ignored --nocapture"]
fn sequential_upload_checkpoints() {
    const CHUNKS: usize = 4096;
    println!("task,iterations,min_ms,avg_ms,p50_ms,p95_ms,p99_ms");
    measure("legacy_4096_chunks", || {
        let mut completed = Vec::new();
        for chunk in 0..CHUNKS {
            if !completed.contains(&chunk) {
                completed.push(chunk);
                completed.sort_unstable();
            }
            std::hint::black_box(serde_json::to_string(&completed).unwrap());
        }
        assert_eq!(completed.len(), CHUNKS);
    });
    measure("compact_4096_chunks", || {
        let mut state =
            ResumableUploadState::new("benchmark".into(), "file".into(), CHUNKS as u64, 1);
        for chunk in 0..CHUNKS {
            state.mark_chunk_complete(chunk);
            std::hint::black_box(state.checkpoint().unwrap());
        }
        assert!(state.is_complete());
    });
}
