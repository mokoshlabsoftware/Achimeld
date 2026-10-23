/// Last completed output survives invalidation, queued work and failed renders.
/// Completion must match the most recently requested generation.
pub struct PreviewState<T> {
    requested: Option<u64>,
    completed: Option<(u64, T)>,
    error: Option<String>,
}
impl<T> Default for PreviewState<T> {
    fn default() -> Self {
        Self {
            requested: None,
            completed: None,
            error: None,
        }
    }
}
impl<T> PreviewState<T> {
    pub fn request(&mut self, generation: u64) -> bool {
        if self.requested == Some(generation) {
            false
        } else {
            self.requested = Some(generation);
            self.error = None;
            true
        }
    }
    pub fn complete(&mut self, generation: u64, result: Result<T, String>) -> bool {
        if self.requested != Some(generation) {
            return false;
        }
        match result {
            Ok(output) => {
                self.completed = Some((generation, output));
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }
    /// Queue admission failed; allow the host to retry without clearing the last image.
    pub fn defer(&mut self, generation: u64) {
        if self.requested == Some(generation) {
            self.requested = None;
        }
    }
    pub fn output(&self) -> Option<&T> {
        self.completed.as_ref().map(|(_, output)| output)
    }
    pub fn generation(&self) -> Option<u64> {
        self.completed.as_ref().map(|(generation, _)| *generation)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_results_and_failed_renders_never_flash_or_clear_the_preview() {
        let mut preview = PreviewState::default();
        assert!(preview.request(1));
        assert!(preview.complete(1, Ok("first")));
        assert!(preview.request(2));
        assert_eq!(preview.output(), Some(&"first"));
        assert!(preview.request(3));
        assert!(!preview.complete(2, Ok("obsolete")));
        assert_eq!(preview.output(), Some(&"first"));
        assert!(!preview.complete(3, Err("failed".into())));
        assert_eq!(preview.output(), Some(&"first"));
        assert!(preview.request(4));
        assert!(preview.complete(4, Ok("current")));
        assert_eq!(preview.output(), Some(&"current"));
        assert!(!preview.complete(1, Ok("late first")));
        assert_eq!(preview.output(), Some(&"current"));
    }
    #[test]
    fn full_queue_can_retry_without_clearing_or_accepting_unsubmitted_output() {
        let mut preview = PreviewState::default();
        preview.request(1);
        preview.complete(1, Ok("first"));
        assert!(preview.request(2));
        preview.defer(2);
        assert!(!preview.complete(2, Ok("unsubmitted")));
        assert_eq!(preview.output(), Some(&"first"));
        assert!(preview.request(2));
        assert!(preview.complete(2, Ok("second")));
    }
}
