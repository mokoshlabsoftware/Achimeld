use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_PROJECTS: usize = 10;
const MAX_STATE_BYTES: usize = 2 * 1024 * 1024;

// Application navigation state is separate from editable preferences/undo.
// Paths are data: opening still goes through ProjectStore's normal validation.
pub(crate) struct RecentProjects {
    pub(crate) paths: Vec<PathBuf>,
    state_path: Option<PathBuf>,
    exclude_test_projects: bool,
}

impl RecentProjects {
    pub(crate) fn new(state_path: Option<PathBuf>) -> Self {
        let paths = state_path
            .as_deref()
            .and_then(|path| load(path).ok())
            .unwrap_or_default();
        Self {
            paths,
            state_path,
            exclude_test_projects: false,
        }
    }

    pub(crate) fn for_user(state_path: Option<PathBuf>) -> Self {
        let mut recent = Self::new(state_path);
        recent.exclude_test_projects = true;
        // Clean up history written by older test hosts, even after their files
        // have been deleted. Ordinary missing projects remain available.
        recent.paths.retain(|path| !is_generated_test_project(path));
        recent
    }

    pub(crate) fn remember(&mut self, path: &Path) -> Result<(), String> {
        validate_path(path)?;
        if self.exclude_test_projects && is_generated_test_project(path) {
            return Ok(());
        }
        self.paths.retain(|old| !same_path(old, path));
        self.paths.insert(0, path.to_owned());
        self.paths.truncate(MAX_PROJECTS);
        let state_path = self
            .state_path
            .as_deref()
            .ok_or("Application settings directory unavailable")?;
        let bytes = serde_json::to_vec_pretty(&self.paths).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err("Recent project state is too large".into());
        }
        std::fs::create_dir_all(
            state_path
                .parent()
                .ok_or("Settings directory unavailable")?,
        )
        .map_err(|e| e.to_string())?;
        achimeld_persistence::project::write_atomic(state_path, &bytes)
    }
}

fn is_generated_test_project(path: &Path) -> bool {
    let artifacts = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("ACHIMELD_BUILD_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Projects\AchimeldBuild"))
                .join("current")
        });
    let temp = std::env::temp_dir().join("achimeld-tests");
    path.ancestors().skip(1).any(|folder| {
        let Some(parent) = folder.parent() else {
            return false;
        };
        if !same_path(parent, &artifacts) && !same_path(parent, &temp) {
            return false;
        }
        let Some(name) = folder.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let name = name.to_ascii_lowercase();
        [
            "translation-host-",
            "sync-origin-",
            "recent-host-",
            "art-bank-",
        ]
        .iter()
        .filter_map(|prefix| name.strip_prefix(prefix))
        .any(|id| {
            id.len() == 36
                && id.bytes().enumerate().all(|(index, byte)| {
                    if [8, 13, 18, 23].contains(&index) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                })
        })
    })
}

fn same_path(first: &Path, second: &Path) -> bool {
    first == second
        || first
            .to_str()
            .zip(second.to_str())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn validate_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path.to_str().is_none_or(|s| s.len() > 128 * 1024)
        || !path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("achv"))
    {
        return Err("Invalid recent project path".into());
    }
    Ok(())
}

fn load(path: &Path) -> Result<Vec<PathBuf>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err("Recent project state is too large".into());
    }
    let paths: Vec<PathBuf> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if paths.len() > MAX_PROJECTS {
        return Err("Too many recent projects".into());
    }
    let mut unique: Vec<PathBuf> = Vec::new();
    for path in paths {
        validate_path(&path)?;
        if !unique.iter().any(|old| same_path(old, &path)) {
            unique.push(path);
        }
    }
    Ok(unique)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_history_excludes_generated_fixtures_and_preserves_real_project_paths() {
        let root = crate::test_output::root()
            .join(format!("recent-cleanup-{}", achimeld_application::new_id()));
        let state = root.join("recent.json");
        let fixture = std::env::temp_dir()
            .join("achimeld-tests")
            .join(format!(
                "translation-host-{}",
                achimeld_application::new_id()
            ))
            .join("translated.achv");
        let ordinary = root.join("test-project.achv");
        let similar_name = root
            .join(format!(
                "translation-host-{}",
                achimeld_application::new_id()
            ))
            .join("translated.achv");
        let mut old_history = RecentProjects::new(Some(state.clone()));
        for path in [&ordinary, &fixture, &similar_name] {
            old_history.remember(path).unwrap();
        }
        let mut recent = RecentProjects::for_user(Some(state.clone()));
        assert_eq!(recent.paths, vec![similar_name.clone(), ordinary.clone()]);
        recent.remember(&fixture).unwrap();
        assert_eq!(recent.paths, vec![similar_name.clone(), ordinary.clone()]);
        recent.remember(&ordinary).unwrap();
        assert_eq!(
            RecentProjects::new(Some(state.clone())).paths,
            vec![ordinary, similar_name]
        );
        std::fs::remove_file(state).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn history_survives_restart_deduplicates_and_retains_ten_newest_paths() {
        let root =
            crate::test_output::root().join(format!("recent-{}", achimeld_application::new_id()));
        let state = root.join("recent.json");
        let mut recent = RecentProjects::new(Some(state.clone()));
        for i in 0..12 {
            recent
                .remember(&root.join(format!("Project-{i}.achv")))
                .unwrap();
        }
        assert_eq!(recent.paths.len(), 10);
        assert_eq!(recent.paths[0], root.join("Project-11.achv"));
        assert_eq!(recent.paths[9], root.join("Project-2.achv"));
        recent.remember(&root.join("PROJECT-4.ACHV")).unwrap();
        assert_eq!(recent.paths.len(), 10);
        assert_eq!(recent.paths[0], root.join("PROJECT-4.ACHV"));
        assert_eq!(RecentProjects::new(Some(state.clone())).paths, recent.paths);
        // These paths need not exist: stale entries stay visible for explicit reopening.
        std::fs::remove_file(state).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn malformed_oversized_or_non_project_state_is_ignored() {
        let root = crate::test_output::root()
            .join(format!("recent-invalid-{}", achimeld_application::new_id()));
        std::fs::create_dir_all(&root).unwrap();
        let state = root.join("recent.json");
        for bytes in [
            b"invalid".to_vec(),
            serde_json::to_vec(&vec![root.join("project.achv"); 11]).unwrap(),
            serde_json::to_vec(&vec![PathBuf::from("relative.achv")]).unwrap(),
            serde_json::to_vec(&vec![root.join("program.exe")]).unwrap(),
            vec![b' '; MAX_STATE_BYTES + 1],
        ] {
            std::fs::write(&state, bytes).unwrap();
            assert!(load(&state).is_err());
            assert!(RecentProjects::new(Some(state.clone())).paths.is_empty());
        }
        std::fs::remove_file(state).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
