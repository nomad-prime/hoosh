use crate::agent::{Attachment, AttachmentKind};
use crate::tools::{Tool, file_ops::ReadFileTool};
use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

/// Image extensions that get attached instead of inlined.
const IMAGE_EXTENSIONS: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

fn image_media_type(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_EXTENSIONS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, m)| *m)
}

#[derive(Debug, Clone)]
pub struct FileReference {
    pub original_text: String,
    pub file_path: String,
    pub line_range: Option<(usize, usize)>,
}

pub struct MessageParser {
    working_directory: PathBuf,
    read_file_tool: ReadFileTool,
}

impl MessageParser {
    pub fn new() -> Self {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            read_file_tool: ReadFileTool::with_working_directory(working_dir.clone()),
            working_directory: working_dir,
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            read_file_tool: ReadFileTool::with_working_directory(working_dir.clone()),
            working_directory: working_dir,
        }
    }

    pub fn find_file_references(&self, message: &str) -> Result<Vec<FileReference>> {
        // Regex to match @filename patterns with optional line ranges
        // Supports: @file.txt, @src/main.rs, @file.txt:10-20, @file.txt:15
        let re = Regex::new(r"@([^\s@:]+(?:\.[^\s@:]+)*)(:\d+(?:-\d+)?)?")
            .context("Failed to compile file reference regex")?;

        let mut references = Vec::new();

        for captures in re.captures_iter(message) {
            let full_match_obj = captures.get(0).expect("Regex match should have full match");
            let full_match = full_match_obj.as_str();
            let match_start = full_match_obj.start();

            // Check if the @ is preceded by an alphanumeric character (indicating an email)
            if match_start > 0 {
                let preceding_char = message.chars().nth(match_start - 1);
                if let Some(ch) = preceding_char
                    && ch.is_alphanumeric()
                {
                    continue; // Skip this match as it's likely an email
                }
            }

            let file_path = captures
                .get(1)
                .expect("Regex match should have file path group")
                .as_str();
            let line_spec = captures.get(2).map(|m| m.as_str());

            let line_range = if let Some(line_spec) = line_spec {
                Self::parse_line_range(&line_spec[1..])? // Remove the ':' prefix
            } else {
                None
            };

            references.push(FileReference {
                original_text: full_match.to_string(),
                file_path: file_path.to_string(),
                line_range,
            });
        }

        Ok(references)
    }

    fn parse_line_range(line_spec: &str) -> Result<Option<(usize, usize)>> {
        if line_spec.is_empty() {
            return Ok(None);
        }

        if line_spec.contains('-') {
            let parts: Vec<&str> = line_spec.split('-').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid line range format: {}", line_spec);
            }
            let start: usize = parts[0]
                .parse()
                .with_context(|| format!("Invalid start line number: {}", parts[0]))?;
            let end: usize = parts[1]
                .parse()
                .with_context(|| format!("Invalid end line number: {}", parts[1]))?;

            if start > end {
                anyhow::bail!(
                    "Start line ({}) cannot be greater than end line ({})",
                    start,
                    end
                );
            }

            Ok(Some((start, end)))
        } else {
            let line: usize = line_spec
                .parse()
                .with_context(|| format!("Invalid line number: {}", line_spec))?;
            Ok(Some((line, line)))
        }
    }

    pub async fn read_file_reference(&self, file_ref: &FileReference) -> Result<String> {
        let mut args = serde_json::json!({
            "path": file_ref.file_path
        });

        if let Some((start, end)) = file_ref.line_range {
            args["start_line"] = serde_json::Value::Number(serde_json::Number::from(start));
            args["end_line"] = serde_json::Value::Number(serde_json::Number::from(end));
        }

        let context = crate::tools::ToolExecutionContext {
            tool_call_id: "parser".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };
        self.read_file_tool
            .execute(&args, &context)
            .await
            .map_err(Into::into)
    }

    pub async fn expand_message(&self, message: &str) -> Result<String> {
        let (expanded, _) = self.expand_message_with_attachments(message).await?;
        Ok(expanded)
    }

    /// Like [`Self::expand_message`], but image-extension `@` refs (`.png`,
    /// `.jpg`, ...) are read as bytes and returned as attachments. The text
    /// reference is replaced by an `[image #N]` marker so the model still has
    /// a positional anchor.
    pub async fn expand_message_with_attachments(
        &self,
        message: &str,
    ) -> Result<(String, Vec<Attachment>)> {
        let file_references = self.find_file_references(message)?;

        if file_references.is_empty() {
            return Ok((message.to_string(), Vec::new()));
        }

        let mut expanded_message = message.to_string();
        let mut file_contents = Vec::new();
        let mut attachments: Vec<Attachment> = Vec::new();

        for file_ref in &file_references {
            if let Some(media_type) = image_media_type(&file_ref.file_path) {
                match self.read_image_bytes(&file_ref.file_path) {
                    Ok(data) => {
                        attachments.push(Attachment {
                            kind: AttachmentKind::Image,
                            media_type: media_type.to_string(),
                            data,
                        });
                        let marker = format!("[image #{}]", attachments.len());
                        // Replace the first occurrence of this @ref with the marker.
                        if let Some(pos) = expanded_message.find(&file_ref.original_text) {
                            expanded_message
                                .replace_range(pos..pos + file_ref.original_text.len(), &marker);
                        }
                    }
                    Err(e) => {
                        file_contents.push(format!(
                            "\n\n--- Error reading image {}: {}",
                            file_ref.file_path, e
                        ));
                    }
                }
                continue;
            }

            match self.read_file_reference(file_ref).await {
                Ok(content) => {
                    let line_info = if let Some((start, end)) = file_ref.line_range {
                        if start == end {
                            format!(" (line {})", start)
                        } else {
                            format!(" (lines {}-{})", start, end)
                        }
                    } else {
                        String::new()
                    };

                    file_contents.push(format!(
                        "\n\n--- Content of {}{}:\n```\n{}\n```",
                        file_ref.file_path, line_info, content
                    ));
                }
                Err(e) => {
                    file_contents.push(format!(
                        "\n\n--- Error reading {}: {}",
                        file_ref.file_path, e
                    ));
                }
            }
        }

        for content in file_contents {
            expanded_message.push_str(&content);
        }

        Ok((expanded_message, attachments))
    }

    fn read_image_bytes(&self, file_path: &str) -> Result<Vec<u8>> {
        let path = Path::new(file_path);
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };
        std::fs::read(&full_path)
            .with_context(|| format!("Failed to read image at {}", full_path.display()))
    }

    pub fn validate_file_path(&self, file_path: &str) -> Result<PathBuf> {
        let path = Path::new(file_path);
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };

        // Security check: ensure we're not accessing outside the working directory
        if !full_path.starts_with(&self.working_directory) {
            anyhow::bail!("Access denied: cannot access files outside working directory");
        }

        if !full_path.exists() {
            anyhow::bail!("File does not exist: {}", full_path.display());
        }

        Ok(full_path)
    }

    pub fn get_file_summary(&self, message: &str) -> Result<String> {
        let file_references = self.find_file_references(message)?;

        if file_references.is_empty() {
            return Ok("No file references found.".to_string());
        }

        let mut summary = format!("Found {} file reference(s):\n", file_references.len());

        for (i, file_ref) in file_references.iter().enumerate() {
            let line_info = if let Some((start, end)) = file_ref.line_range {
                if start == end {
                    format!(" (line {})", start)
                } else {
                    format!(" (lines {}-{})", start, end)
                }
            } else {
                String::new()
            };

            summary.push_str(&format!(
                "  {}. {}{}\n",
                i + 1,
                file_ref.file_path,
                line_info
            ));
        }

        Ok(summary)
    }
}

impl Default for MessageParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_find_file_references() {
        let parser = MessageParser::new();

        let message = "Please review @src/main.rs and also check @config.toml:10-20";
        let refs = parser
            .find_file_references(message)
            .expect("Should find file references");

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].file_path, "src/main.rs");
        assert_eq!(refs[0].line_range, None);
        assert_eq!(refs[1].file_path, "config.toml");
        assert_eq!(refs[1].line_range, Some((10, 20)));
    }

    #[tokio::test]
    async fn test_parse_line_range() {
        assert_eq!(
            MessageParser::parse_line_range("").expect("Should parse empty range"),
            None
        );
        assert_eq!(
            MessageParser::parse_line_range("10").expect("Should parse single line"),
            Some((10, 10))
        );
        assert_eq!(
            MessageParser::parse_line_range("10-20").expect("Should parse line range"),
            Some((10, 20))
        );

        assert!(MessageParser::parse_line_range("20-10").is_err()); // Invalid range
        assert!(MessageParser::parse_line_range("abc").is_err()); // Invalid number
    }

    #[tokio::test]
    async fn test_expand_message() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, World!\nLine 2\nLine 3";

        fs::write(&test_file, content).await.unwrap();

        let parser = MessageParser::with_working_directory(temp_dir.path().to_path_buf());
        let message = "Please review @test.txt";

        let expanded = parser.expand_message(message).await.unwrap();
        assert!(expanded.contains("Hello, World!"));
        assert!(expanded.contains("Content of test.txt:"));
    }

    #[tokio::test]
    async fn image_ref_produces_attachment_and_marker() {
        let temp_dir = tempdir().unwrap();
        let png = temp_dir.path().join("shot.png");
        let bytes = b"\x89PNG\r\n\x1a\nFAKE";
        fs::write(&png, bytes).await.unwrap();

        let parser = MessageParser::with_working_directory(temp_dir.path().to_path_buf());
        let (expanded, attachments) = parser
            .expand_message_with_attachments("describe @shot.png please")
            .await
            .unwrap();

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].kind, AttachmentKind::Image);
        assert_eq!(attachments[0].media_type, "image/png");
        assert_eq!(attachments[0].data, bytes);
        assert!(expanded.contains("[image #1]"));
        assert!(!expanded.contains("@shot.png"));
    }

    #[tokio::test]
    async fn non_image_ref_still_inlines() {
        let temp_dir = tempdir().unwrap();
        let txt = temp_dir.path().join("notes.txt");
        fs::write(&txt, "hello").await.unwrap();

        let parser = MessageParser::with_working_directory(temp_dir.path().to_path_buf());
        let (expanded, attachments) = parser
            .expand_message_with_attachments("see @notes.txt")
            .await
            .unwrap();

        assert!(attachments.is_empty());
        assert!(expanded.contains("hello"));
    }

    #[test]
    fn test_file_reference_patterns() {
        let parser = MessageParser::new();

        let test_cases = vec![
            ("@file.txt", vec!["file.txt"]),
            ("@src/main.rs", vec!["src/main.rs"]),
            ("@file.txt:10", vec!["file.txt"]),
            ("@file.txt:10-20", vec!["file.txt"]),
            ("Review @a.txt and @b.txt", vec!["a.txt", "b.txt"]),
            ("@path/to/file.ext:5-15", vec!["path/to/file.ext"]),
            ("No files here", vec![]),
            ("Email test@example.com", vec![]), // Should not match email
        ];

        for (message, expected_files) in test_cases {
            let refs = parser.find_file_references(message).unwrap();
            let found_files: Vec<String> = refs.iter().map(|r| r.file_path.clone()).collect();
            assert_eq!(
                found_files, expected_files,
                "Failed for message: {}",
                message
            );
        }
    }
}
