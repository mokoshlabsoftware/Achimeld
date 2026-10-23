/// A drag uses the original ID order and row geometry throughout the gesture.
/// Painting the insertion marker never mutates the model or changes its hit targets.
#[derive(Debug, Clone)]
pub struct ReorderGesture {
    original: Vec<String>,
    dragged: String,
    centers: Vec<f32>,
    slot: usize,
}
impl ReorderGesture {
    pub fn begin(ids: Vec<String>, centers: Vec<f32>, dragged: &str) -> Option<Self> {
        if ids.len() != centers.len()
            || centers.windows(2).any(|v| v[0] >= v[1])
            || centers.iter().any(|v| !v.is_finite())
        {
            return None;
        }
        let slot = ids.iter().position(|id| id == dragged)?;
        Some(Self {
            original: ids,
            centers,
            dragged: dragged.into(),
            slot,
        })
    }
    pub fn update(&mut self, y: f32) -> usize {
        if !y.is_finite() {
            return self.slot;
        }
        let candidate = self.centers.iter().filter(|center| y > **center).count();
        // Dead band around row midpoints prevents adjacent-slot oscillation.
        if candidate != self.slot {
            let boundary = if candidate > self.slot {
                self.centers.get(candidate.saturating_sub(1))
            } else {
                self.centers.get(candidate)
            };
            if boundary.is_none_or(|b| (y - *b).abs() > 5.0) {
                self.slot = candidate;
            }
        }
        self.slot
    }
    pub fn slot(&self) -> usize {
        self.slot
    }
    pub fn finish(self, current: &[String]) -> Result<Vec<String>, String> {
        if self.original != current {
            return Err("Layer order changed during the drag; drag was cancelled".into());
        }
        let old = self
            .original
            .iter()
            .position(|id| id == &self.dragged)
            .ok_or("Dragged layer disappeared")?;
        let mut result = self.original;
        result.remove(old);
        let slot = if self.slot > old {
            self.slot - 1
        } else {
            self.slot
        };
        result.insert(slot.min(result.len()), self.dragged);
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_pointer_motion_never_changes_model_and_drop_commits_once() {
        let ids = vec!["a".into(), "b".into(), "c".into()];
        let mut drag = ReorderGesture::begin(ids.clone(), vec![20.0, 60.0, 100.0], "a").unwrap();
        drag.update(70.0);
        for y in [59.0, 61.0, 58.0, 62.0] {
            assert_eq!(drag.update(y), 2);
        }
        assert_eq!(drag.finish(&ids).unwrap(), vec!["b", "a", "c"]);
    }
    #[test]
    fn concurrent_model_changes_cancel_the_drag() {
        let mut drag =
            ReorderGesture::begin(vec!["a".into(), "b".into()], vec![10.0, 40.0], "a").unwrap();
        drag.update(80.0);
        assert!(drag.finish(&["b".into(), "a".into()]).is_err());
    }
}
