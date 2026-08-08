use std::{
    cmp::Reverse,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::Context;
use chrono::{DateTime, Local};
use size_format::SizeFormatterBinary as SF;

/// A completed download found in the default output folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedItem {
    /// Full path of the item on disk.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Last modified time.
    pub modified: SystemTime,
}

/// Recursively scan `dir` and return every regular file inside it.
///
/// Returns an empty list when `dir` does not exist or is not readable.
pub fn list_completed_downloads(dir: &Path) -> anyhow::Result<Vec<CompletedItem>> {
    let mut items = Vec::new();
    if !dir.is_dir() {
        return Ok(items);
    }
    scan_dir(dir, &mut items).with_context(|| format!("error scanning {}", dir.display()))?;
    items.sort_by_key(|b| Reverse(b.modified));
    Ok(items)
}

fn scan_dir(dir: &Path, items: &mut Vec<CompletedItem>) -> anyhow::Result<()> {
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("error reading {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            scan_dir(&path, items)?;
        } else if file_type.is_file() {
            let meta = entry.metadata()?;
            items.push(CompletedItem {
                path: path.clone(),
                name: entry.file_name().to_string_lossy().into_owned(),
                size: meta.len(),
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(())
}

/// Format a completed item as a single display line: `name (size, date)`.
pub fn format_completed(item: &CompletedItem) -> String {
    let modified: DateTime<Local> = item.modified.into();
    format!(
        "{} ({}, {})",
        item.name,
        SF::new(item.size),
        modified.format("%d/%m/%Y %H:%M")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn returns_empty_when_directory_does_not_exist() {
        let result =
            list_completed_downloads(Path::new("/nonexistent/definitely/missing")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn lists_files_recursively_sorted_by_newest() {
        use std::fs::{File, FileTimes};
        use std::time::{Duration, SystemTime};

        let temp = std::env::temp_dir().join(format!("opentorrent-test-{}", std::process::id()));
        let sub = temp.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let old = temp.join("old.txt");
        let new = sub.join("new.bin");
        fs::write(&old, b"old").unwrap();
        fs::write(&new, vec![0u8; 512]).unwrap();

        // Force distinct modification times so the sort order is deterministic.
        let now = SystemTime::now();
        let _ = File::open(&old)
            .unwrap()
            .set_times(FileTimes::new().set_modified(now - Duration::from_secs(120)));
        let _ = File::open(&new)
            .unwrap()
            .set_times(FileTimes::new().set_modified(now));

        let result = list_completed_downloads(&temp).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|i| i.name == "old.txt" && i.size == 3));
        assert!(result.iter().any(|i| i.name == "new.bin" && i.size == 512));
        // newest first
        assert_eq!(result[0].name, "new.bin");

        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn skips_directories() {
        let temp = std::env::temp_dir().join(format!("opentorrent-dir-{}", std::process::id()));
        fs::create_dir_all(temp.join("empty-dir")).unwrap();
        fs::write(temp.join("file.txt"), b"x").unwrap();

        let result = list_completed_downloads(&temp).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "file.txt");

        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn formats_item_with_name_size_and_date() {
        let item = CompletedItem {
            path: PathBuf::from("/tmp/x.iso"),
            name: "x.iso".into(),
            size: 700 * 1024 * 1024,
            modified: SystemTime::UNIX_EPOCH,
        };
        let line = format_completed(&item);
        assert!(line.starts_with("x.iso ("));
        assert!(line.contains("700"));
    }
}
