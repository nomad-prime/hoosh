use crate::commands::custom::metadata::CommandMetadata;
use anyhow::{Context, Result, anyhow};
use gray_matter::Matter;
use gray_matter::engine::YAML;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub metadata: CommandMetadata,
    pub body: String,
}

pub fn parse_command_file(file_path: &Path) -> Result<ParsedCommand> {
    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid filename: {}", file_path.display()))?
        .to_string();

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read command file: {}", file_path.display()))?;

    let matter = Matter::<YAML>::new();
    let result = matter
        .parse_with_struct::<CommandMetadata>(&content)
        .with_context(|| format!("Failed to parse frontmatter in: {}", file_path.display()))?;

    validate_metadata(&result.data, file_path)?;

    Ok(ParsedCommand {
        name,
        metadata: result.data,
        body: result.content.trim().to_string(),
    })
}

fn validate_metadata(metadata: &CommandMetadata, file_path: &Path) -> Result<()> {
    if metadata.description.trim().is_empty() {
        anyhow::bail!(
            "Command file '{}' has empty description",
            file_path.display()
        );
    }

    for (idx, handoff) in metadata.handoffs.iter().enumerate() {
        if handoff.agent.trim().is_empty() {
            anyhow::bail!(
                "Handoff #{} in '{}' is missing agent name",
                idx + 1,
                file_path.display()
            );
        }
        if handoff.label.trim().is_empty() {
            anyhow::bail!(
                "Handoff #{} in '{}' is missing label",
                idx + 1,
                file_path.display()
            );
        }
    }

    Ok(())
}
