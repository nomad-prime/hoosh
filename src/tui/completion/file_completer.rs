use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;

use super::Completer;

pub struct FileCompleter {
    working_directory: PathBuf,
    cache: std::sync::Arc<tokio::sync::RwLock<Vec<PathBuf>>>,
}

impl FileCompleter {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            cache: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    async fn scan_directory(&self, dir: &Path, max_depth: usize) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.scan_directory_recursive(dir, dir, max_depth, 0, &mut files)
            .await?;
        Ok(files)
    }

    fn scan_directory_recursive<'a>(
        &'a self,
        base_dir: &'a Path,
        current_dir: &'a Path,
        max_depth: usize,
        current_depth: usize,
        files: &'a mut Vec<PathBuf>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if current_depth > max_depth {
                return Ok(());
            }

            let mut entries = fs::read_dir(current_dir).await?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Skip hidden files and common ignore patterns
                if file_name.starts_with('.')
                    || file_name == "target"
                    || file_name == "node_modules"
                {
                    continue;
                }

                let metadata = entry.metadata().await?;

                if metadata.is_file() {
                    // Store relative path from working directory
                    if let Ok(relative) = path.strip_prefix(base_dir) {
                        files.push(relative.to_path_buf());
                    }
                } else if metadata.is_dir() {
                    self.scan_directory_recursive(
                        base_dir,
                        &path,
                        max_depth,
                        current_depth + 1,
                        files,
                    )
                    .await?;
                }
            }

            Ok(())
        })
    }

    async fn refresh_cache(&self) -> Result<()> {
        let files = self.scan_directory(&self.working_directory, 5).await?;
        let mut cache = self.cache.write().await;
        *cache = files;
        Ok(())
    }

    fn fuzzy_match(pattern: &str, target: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }

        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        // Simple fuzzy matching: all pattern chars must appear in order
        let mut pattern_chars = pattern_lower.chars();
        let mut current_pattern_char = pattern_chars.next();

        for target_char in target_lower.chars() {
            if let Some(pc) = current_pattern_char {
                if pc == target_char {
                    current_pattern_char = pattern_chars.next();
                }
            }
        }

        current_pattern_char.is_none()
    }

    fn score_match(pattern: &str, target: &str) -> i32 {
        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        let mut score = 0;

        // Exact match gets highest score
        if target_lower == pattern_lower {
            return 1000;
        }

        // Starts with pattern gets high score
        if target_lower.starts_with(&pattern_lower) {
            score += 500;
        }

        // Contains pattern as substring
        if target_lower.contains(&pattern_lower) {
            score += 300;
        }

        // Fuzzy match gets base score
        if Self::fuzzy_match(pattern, target) {
            score += 100;
        }

        // Shorter paths are preferred
        score -= target.len() as i32;

        score
    }
}

#[async_trait]
impl Completer for FileCompleter {
    fn trigger_key(&self) -> char {
        '@'
    }

    async fn get_completions(&self, query: &str) -> Result<Vec<String>> {
        // Refresh cache on first use or if empty
        {
            let cache = self.cache.read().await;
            if cache.is_empty() {
                drop(cache);
                let _ = self.refresh_cache().await;
            }
        }

        let cache = self.cache.read().await;

        let mut matches: Vec<(String, i32)> = cache
            .iter()
            .filter_map(|path| {
                let path_str = path.to_string_lossy().to_string();
                if Self::fuzzy_match(query, &path_str) {
                    let score = Self::score_match(query, &path_str);
                    Some((path_str, score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending)
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        // Return top 50 matches
        Ok(matches.into_iter().take(50).map(|(path, _)| path).collect())
    }

    fn format_completion(&self, item: &str) -> String {
        item.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(FileCompleter::fuzzy_match("src", "src/main.rs"));
        assert!(FileCompleter::fuzzy_match("smai", "src/main.rs"));
        assert!(FileCompleter::fuzzy_match("", "anything"));
        assert!(!FileCompleter::fuzzy_match("xyz", "abc"));
    }

    #[test]
    fn test_score_match() {
        // Exact match should score highest
        assert!(
            FileCompleter::score_match("test", "test")
                > FileCompleter::score_match("test", "test.rs")
        );

        // Prefix match should score higher than fuzzy
        assert!(
            FileCompleter::score_match("src", "src/main.rs")
                > FileCompleter::score_match("src", "source/main.rs")
        );

        // Shorter paths should score higher
        assert!(
            FileCompleter::score_match("test", "test.rs")
                > FileCompleter::score_match("test", "long/path/to/test.rs")
        );
    }
}
