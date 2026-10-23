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

pub(crate) fn current_version() -> &'static str {
    ENTRIES.first().expect("current working version").version
}

#[cfg(test)]
mod tests {
    use super::*;

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
