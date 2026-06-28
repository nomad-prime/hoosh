use super::app_state::ActiveToolCall;
use crate::tools::CategoryPhrasing;

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn aggregate_phrase(calls: &[ActiveToolCall]) -> String {
    phrase_with(calls, |p| p.gerund)
}

pub fn completed_phrase(calls: &[ActiveToolCall]) -> String {
    phrase_with(calls, |p| p.past)
}

fn phrase_with(calls: &[ActiveToolCall], verb: fn(&CategoryPhrasing) -> &'static str) -> String {
    let mut counts: Vec<(CategoryPhrasing, usize)> = Vec::new();

    for call in calls {
        if let Some(entry) = counts.iter_mut().find(|(p, _)| *p == call.phrasing) {
            entry.1 += 1;
        } else {
            counts.push((call.phrasing, 1));
        }
    }

    let segments: Vec<String> = counts
        .iter()
        .map(|(p, count)| format!("{} {} {}", verb(p), count, p.noun(*count)))
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
