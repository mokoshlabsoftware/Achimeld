//! A separate thread observes active UI work; idle/minimized windows are not hangs.
use super::output;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Activity {
    started: Option<Instant>,
    phase: &'static str,
    reported: bool,
}
impl Activity {
    fn warning(&mut self, now: Instant) -> Option<String> {
        let elapsed = now.checked_duration_since(self.started?)?;
        if elapsed < Duration::from_secs(5) || self.reported {
            return None;
        }
        self.reported = true;
        Some(format!(
            "UI work has not completed for {} ms; phase={}",
            elapsed.as_millis(),
            self.phase
        ))
    }
}

pub struct Monitor {
    activity: Arc<Mutex<Activity>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Monitor {
    pub fn new() -> Self {
        let activity = Arc::new(Mutex::new(Activity::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let observed = activity.clone();
        let stopped = stop.clone();
        let thread = std::thread::Builder::new()
            .name("diagnostic-monitor".into())
            .spawn(move || {
                let mut next_retry = Instant::now();
                let mut last_failure = None;
                while !stopped.load(Ordering::Acquire) {
                    let now = Instant::now();
                    let warning = observed
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .warning(now);
                    if let Some(warning) = warning {
                        output::record("ERROR", "responsiveness", &warning);
                    }
                    if now >= next_retry {
                        let failure = output::persistence_status().failure;
                        if failure != last_failure
                            && let Some(message) = &failure
                        {
                            output::record("ERROR", "diagnostics", message);
                        }
                        let unavailable = failure.is_some();
                        last_failure = failure;
                        super::retry_persistence();
                        if unavailable && output::persistence_status().failure.is_none() {
                            output::record(
                                "INFO",
                                "diagnostics",
                                "File logging restored; retained session output replayed",
                            );
                        }
                        next_retry = now + Duration::from_secs(10);
                    }
                    std::thread::park_timeout(Duration::from_millis(500));
                }
            })
            .map_err(|_| {
                output::record("ERROR", "diagnostics", "Could not start diagnostic monitor")
            })
            .ok();
        Self {
            activity,
            stop,
            thread,
        }
    }

    pub fn frame(&self) -> Frame {
        *self.activity.lock().unwrap_or_else(|e| e.into_inner()) = Activity {
            started: Some(Instant::now()),
            phase: "begin frame",
            reported: false,
        };
        Frame(self.activity.clone())
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

pub struct Frame(Arc<Mutex<Activity>>);
impl Frame {
    pub fn phase(&self, phase: &'static str) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).phase = phase;
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let elapsed = state.started.take().map(|s| s.elapsed());
        let reported = state.reported;
        drop(state);
        if reported && let Some(elapsed) = elapsed {
            output::record(
                "INFO",
                "responsiveness",
                &format!("UI work completed after {} ms", elapsed.as_millis()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn idle_is_not_a_hang_and_active_work_reports_once() {
        let now = Instant::now();
        let mut state = Activity::default();
        assert!(state.warning(now + Duration::from_secs(60)).is_none());
        state.started = Some(now);
        state.phase = "draw editor";
        assert!(state.warning(now + Duration::from_secs(4)).is_none());
        assert!(
            state
                .warning(now + Duration::from_secs(5))
                .unwrap()
                .contains("draw editor")
        );
        assert!(state.warning(now + Duration::from_secs(60)).is_none());
    }
}
