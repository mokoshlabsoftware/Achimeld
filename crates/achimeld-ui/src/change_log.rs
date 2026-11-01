use std::sync::LazyLock;

const SOURCE: &str = include_str!("../../../CHANGELOG.md");

pub(crate) struct Entry {
    pub version: &'static str,
    pub date: &'static str,
    pub changes: Vec<&'static str>,
}

pub(crate) static ENTRIES: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    let mut entries: Vec<Entry> = Vec::new();
    for line in SOURCE.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            let (version, date) = heading
                .strip_suffix(')')
                .and_then(|heading| heading.split_once(" ("))
                .expect("validated embedded change log heading");
            entries.push(Entry {
                version,
                date,
                changes: Vec::new(),
            });
        } else if let Some(change) = line.strip_prefix("- ") {
            entries
                .last_mut()
                .expect("change log entry before its changes")
                .changes
                .push(change);
        }
    }
    entries
});

pub fn current_version() -> &'static str {
    ENTRIES.first().expect("current working version").version
}

fn version_numbers(value: &str) -> Option<[u32; 4]> {
    let parts: Vec<u32> = value
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    parts.try_into().ok()
}

/// A newer acknowledged version is retained when running an older build.
pub fn needs_update_notes(previous: Option<&str>) -> bool {
    previous.and_then(version_numbers).is_none_or(|previous| {
        previous < version_numbers(current_version()).expect("validated current version")
    })
}

pub(crate) fn changes_since(previous: Option<&str>) -> &'static [Entry] {
    let end =
        previous.and_then(|version| ENTRIES.iter().position(|entry| entry.version == version));
    &ENTRIES[..end.unwrap_or(ENTRIES.len())]
}

/// The same compiled identity is used by the visible chrome and native window.
pub fn window_title() -> &'static str {
    static TITLE: LazyLock<String> = LazyLock::new(|| format!("Achimeld {}", current_version()));
    &TITLE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_notes_include_skipped_versions_and_handle_unknown_baselines() {
        assert!(!needs_update_notes(Some(current_version())));
        assert!(!needs_update_notes(Some("4294967295.0.0.0")));
        assert!(needs_update_notes(None));
        assert!(needs_update_notes(Some("invalid")));
        assert!(changes_since(Some(current_version())).is_empty());
        let previous = ENTRIES[2].version;
        assert_eq!(changes_since(Some(previous)).len(), 2);
        assert_eq!(changes_since(Some("unknown")).len(), ENTRIES.len());
        assert_eq!(changes_since(None).len(), ENTRIES.len());
    }

    #[test]
    fn embedded_history_has_four_part_versions_and_dated_changes_newest_first() {
        assert!(!ENTRIES.is_empty());
        let mut previous = None;
        for entry in ENTRIES.iter() {
            let numbers: Vec<u32> = entry
                .version
                .split('.')
                .map(|part| {
                    let number = part.parse::<u32>().expect("numeric version component");
                    assert_eq!(number.to_string(), part, "no leading zeroes");
                    number
                })
                .collect();
            assert_eq!(numbers.len(), 4);
            if let Some(previous) = previous {
                assert!(numbers < previous, "unique versions in descending order");
            }
            previous = Some(numbers);
            assert_eq!(entry.date.len(), 10);
            assert!(!entry.changes.is_empty());
            assert!(entry.changes.iter().all(|change| !change.trim().is_empty()));
        }
        assert_eq!(current_version(), ENTRIES[0].version);
    }
}
