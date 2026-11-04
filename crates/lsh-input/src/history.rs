use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// A trait for managing command history.
pub trait History: Send + Sync {
    /// Returns the number of entries in history.
    fn len(&self) -> usize;

    /// Returns `true` if history contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Retrieve entry at `index` (0 is oldest, `len() - 1` is most recent).
    fn get(&self, index: usize) -> Option<String>;

    /// Append a new command entry to history.
    fn append(&mut self, entry: &str);

    /// Synchronise in-memory history with persistent storage, if applicable.
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<H: History> History for Arc<RwLock<H>> {
    fn len(&self) -> usize {
        self.read().unwrap().len()
    }

    fn get(&self, index: usize) -> Option<String> {
        self.read().unwrap().get(index)
    }

    fn append(&mut self, entry: &str) {
        self.write().unwrap().append(entry);
    }

    fn sync(&mut self) -> io::Result<()> {
        self.write().unwrap().sync()
    }
}

/// An in-memory command history with optional file persistence.
#[derive(Debug, Clone)]
pub struct DefaultHistory {
    entries: Vec<String>,
    max_entries: usize,
    file_path: Option<PathBuf>,
}

impl DefaultHistory {
    /// Create a new in-memory history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries: max_entries.max(1),
            file_path: None,
        }
    }

    /// Create a file-backed history, loading existing entries from `path`.
    pub fn with_file(max_entries: usize, path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let max_entries = max_entries.max(1);
        let mut entries = Vec::new();

        if path.exists() {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line?;
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    entries.push(line);
                }
            }
            if entries.len() > max_entries {
                let excess = entries.len() - max_entries;
                entries.drain(0..excess);
            }
        }

        Ok(Self {
            entries,
            max_entries,
            file_path: Some(path),
        })
    }

    /// Return the list of entries as a slice of strings.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Write all current entries to the backing file.
    pub fn flush_to_file(&self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = File::create(path)?;
            for entry in &self.entries {
                writeln!(file, "{}", entry)?;
            }
            file.flush()?;
        }
        Ok(())
    }
}

impl History for DefaultHistory {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, index: usize) -> Option<String> {
        self.entries.get(index).cloned()
    }

    fn append(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }

        // Avoid adding consecutive duplicate entries
        if let Some(last) = self.entries.last()
            && last == trimmed
        {
            return;
        }

        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(trimmed.to_string());

        // Append to file if persistent
        if let Some(path) = &self.file_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(file, "{}", trimmed);
                let _ = file.flush();
            }
        }
    }

    fn sync(&mut self) -> io::Result<()> {
        self.flush_to_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_history() {
        let mut history = DefaultHistory::new(3);
        assert_eq!(history.len(), 0);
        assert!(history.is_empty());

        history.append("cargo build");
        history.append("cargo test");
        assert_eq!(history.len(), 2);
        assert_eq!(history.get(0).as_deref(), Some("cargo build"));
        assert_eq!(history.get(1).as_deref(), Some("cargo test"));
        assert_eq!(history.get(2), None);

        // Deduplication of consecutive identical entries
        history.append("cargo test");
        assert_eq!(history.len(), 2);

        // Cap at max_entries
        history.append("git status");
        history.append("git diff");
        assert_eq!(history.len(), 3);
        assert_eq!(history.get(0).as_deref(), Some("cargo test"));
        assert_eq!(history.get(1).as_deref(), Some("git status"));
        assert_eq!(history.get(2).as_deref(), Some("git diff"));
    }

    #[test]
    fn test_file_backed_history() {
        let temp_dir = std::env::temp_dir().join(format!("lsh_test_hist_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let path = temp_dir.join("history.txt");

        {
            let mut hist = DefaultHistory::with_file(5, &path).unwrap();
            hist.append("echo first");
            hist.append("echo second");
        }

        // Reopen from disk
        {
            let hist = DefaultHistory::with_file(5, &path).unwrap();
            assert_eq!(hist.len(), 2);
            assert_eq!(hist.get(0).as_deref(), Some("echo first"));
            assert_eq!(hist.get(1).as_deref(), Some("echo second"));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
