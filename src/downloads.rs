use std::{
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

/// Largura da coluna de data do grid de Histórico (US-045): `dd/mm/aaaa hh:mm`.
pub const HISTORY_DATE_W: usize = 16;
/// Largura da coluna de tamanho do grid de Histórico (US-045).
pub const HISTORY_SIZE_W: usize = 14;

/// Recursively scan `dir` and return every regular file inside it.
///
/// Returns an empty list when `dir` does not exist or is not readable.
pub fn list_completed_downloads(dir: &Path) -> anyhow::Result<Vec<CompletedItem>> {
    let mut items = Vec::new();
    if !dir.is_dir() {
        return Ok(items);
    }
    scan_dir(dir, &mut items).with_context(|| format!("error scanning {}", dir.display()))?;
    // Ordenação determinística: mais recente primeiro; empates por nome (para
    // que o grid não "fiquec" reordenando entre frames, US-045).
    items.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.name.cmp(&b.name))
    });
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

/// Trunca `s` para `max` caracteres adicionando "…" quando necessário.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Cabeçalho do grid de Histórico (US-045): colunas DATA | NOME | TAMANHO,
/// com a largura do nome dada por `name_w`.
pub fn history_header(name_w: usize) -> String {
    format!(
        "{:<HISTORY_DATE_W$} {:<name_w$} {:>HISTORY_SIZE_W$}",
        "DATA CRIAÇÃO", "NOME", "TAMANHO"
    )
}

/// Formata um item do histórico em uma linha de colunas alinhadas (US-045):
/// `dd/mm/aaaa hh:mm | nome | tamanho`. O nome é truncado para `name_w`.
pub fn format_completed_row(item: &CompletedItem, name_w: usize) -> String {
    let modified: DateTime<Local> = item.modified.into();
    let date = modified.format("%d/%m/%Y %H:%M");
    let name = truncate(&item.name, name_w);
    // `SizeFormatterBinary` ignora o width do format (não respeita `{:>n}`),
    // então serializamos o tamanho para uma String antes de alinhar.
    let size = SF::new(item.size).to_string();
    format!(
        "{date:<HISTORY_DATE_W$} {:<name_w$} {:>HISTORY_SIZE_W$}",
        name, size
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
        let line = format_completed_row(&item, 40);
        assert!(line.contains("31/12/1969") || line.contains("01/01/1970"));
        assert!(line.contains("x.iso"));
        assert!(line.contains("700"));
    }

    #[test]
    fn history_header_matches_row_width() {
        // Cabeçalho e linha de dados devem ter a mesma largura (colunas
        // alinhadas) para a mesma `name_w` (AC-2/AC-3).
        let item = CompletedItem {
            path: PathBuf::from("/tmp/x.iso"),
            name: "x.iso".into(),
            size: 5 * 1024 * 1024,
            modified: SystemTime::UNIX_EPOCH,
        };
        let name_w = 30;
        let header = history_header(name_w);
        let row = format_completed_row(&item, name_w);
        assert_eq!(header.chars().count(), row.chars().count());
        assert_eq!(
            header.chars().count(),
            HISTORY_DATE_W + 1 + name_w + 1 + HISTORY_SIZE_W
        );
        assert!(header.contains("DATA CRIAÇÃO"));
        assert!(header.contains("TAMANHO"));
    }

    #[test]
    fn format_completed_row_truncates_long_names() {
        let item = CompletedItem {
            path: PathBuf::from("/tmp/x.iso"),
            name: "arquivo-com-nome-muito-longo.iso".into(),
            size: 1,
            modified: SystemTime::UNIX_EPOCH,
        };
        let line = format_completed_row(&item, 10);
        let name_field = line
            .split_whitespace()
            .find(|p| p.contains("…"))
            .unwrap_or_default();
        assert!(name_field.chars().count() <= 10);
        assert!(name_field.contains('…'));
    }
}
