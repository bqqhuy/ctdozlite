//! One queued preview per camera/peer, with stale-call generation rejection.
use std::sync::Mutex;

struct Gate {
    generation: u64,
    pending: [bool; 3],
}
impl Gate {
    const fn new() -> Self {
        Self {
            generation: 0,
            pending: [false; 3],
        }
    }
    fn start(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = [false; 3];
        self.generation
    }
    #[cfg(any(windows, test))]
    fn claim(&mut self, generation: u64, index: usize) -> bool {
        if generation != self.generation || self.pending[index] {
            return false;
        }
        self.pending[index] = true;
        true
    }
    fn consumed(&mut self, generation: u64, index: usize) {
        if generation == self.generation {
            self.pending[index] = false;
        }
    }
}
// The channel carries owned pixels. This short lock only coordinates whether a
// frame is already queued; it never covers decoding, uploading, I/O or UI work.
static GATE: Mutex<Gate> = Mutex::new(Gate::new());
fn index(key: &str) -> Option<usize> {
    match key {
        "call_local_video" => Some(0),
        "call_remote_video" => Some(1),
        "call_remote_video_1" => Some(2),
        _ => None,
    }
}
pub(crate) fn start() -> u64 {
    GATE.lock().unwrap_or_else(|e| e.into_inner()).start()
}
pub(crate) fn current(generation: u64) -> bool {
    GATE.lock().unwrap_or_else(|e| e.into_inner()).generation == generation
}
#[cfg(windows)]
pub(crate) fn claim(generation: u64, key: &str) -> bool {
    #[cfg(windows)]
    if !crate::tray::previews_visible() {
        return false;
    }
    index(key).is_some_and(|index| {
        GATE.lock()
            .unwrap_or_else(|e| e.into_inner())
            .claim(generation, index)
    })
}
pub(crate) fn consumed(generation: u64, key: &str) {
    if let Some(index) = index(key) {
        GATE.lock()
            .unwrap_or_else(|e| e.into_inner())
            .consumed(generation, index);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_ui_cannot_queue_unbounded_frames_and_old_calls_cannot_unlock_new_frames() {
        let mut gate = Gate::new();
        let old = gate.start();
        assert!(gate.claim(old, 0));
        for _ in 0..1000 {
            assert!(!gate.claim(old, 0));
        }
        assert!(gate.claim(old, 1));
        assert!(gate.claim(old, 2));
        assert!(!gate.claim(old, 2));
        let new = gate.start();
        assert!(gate.claim(new, 0));
        gate.consumed(old, 0);
        assert!(!gate.claim(new, 0));
        assert!(!gate.claim(old, 1));
        gate.consumed(new, 0);
        assert!(gate.claim(new, 0));
    }
}
