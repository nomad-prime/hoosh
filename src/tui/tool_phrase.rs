use super::app_state::ActiveToolCall;
use crate::tools::ToolCategory;

fn gerund(category: ToolCategory) -> &'static str {
    match category {
        ToolCategory::Read => "reading",
        ToolCategory::Search => "searching for",
        ToolCategory::Find => "finding",
        ToolCategory::Edit => "editing",
        ToolCategory::List => "listing",
        ToolCategory::Run | ToolCategory::Subagent | ToolCategory::Other => "running",
    }
}

fn noun(category: ToolCategory, count: usize) -> &'static str {
    let (singular, plural) = match category {
        ToolCategory::Read | ToolCategory::Find | ToolCategory::Edit => ("file", "files"),
        ToolCategory::Search => ("pattern", "patterns"),
        ToolCategory::List => ("directory", "directories"),
        ToolCategory::Run => ("command", "commands"),
        ToolCategory::Subagent => ("agent", "agents"),
        ToolCategory::Other => ("tool", "tools"),
    };
    if count == 1 { singular } else { plural }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn aggregate_phrase(calls: &[ActiveToolCall]) -> String {
    let mut counts: Vec<(ToolCategory, usize)> = Vec::new();

    for call in calls {
        if let Some(entry) = counts.iter_mut().find(|(c, _)| *c == call.category) {
            entry.1 += 1;
        } else {
            counts.push((call.category, 1));
        }
    }

    let segments: Vec<String> = counts
        .iter()
        .map(|(cat, count)| format!("{} {} {}", gerund(*cat), count, noun(*cat, *count)))
        .collect();

    capitalize_first(&segments.join(", "))
}

pub fn target_basenames(calls: &[ActiveToolCall]) -> Vec<String> {
    calls
        .iter()
        .map(|call| basename_of_display(&call.display_name))
        .collect()
}

fn basename_of_display(display_name: &str) -> String {
    let inner = display_name
        .find('(')
        .and_then(|start| {
            display_name
                .rfind(')')
                .filter(|end| *end > start)
                .map(|end| &display_name[start + 1..end])
        })
        .unwrap_or(display_name)
        .trim();

    if inner.is_empty() {
        return display_name.to_string();
    }

    inner
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(inner)
        .to_string()
}

#[cfg(test)]
#[path = "tool_phrase_tests.rs"]
mod tests;
