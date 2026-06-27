use super::app_state::ActiveToolCall;

struct Category {
    gerund: &'static str,
    noun_singular: &'static str,
    noun_plural: &'static str,
}

fn category_for(display_name: &str) -> Category {
    let token = display_name.split(['(', ' ']).next().unwrap_or("").trim();

    match token {
        "Read" => Category {
            gerund: "reading",
            noun_singular: "file",
            noun_plural: "files",
        },
        "Grep" | "Search" => Category {
            gerund: "searching for",
            noun_singular: "pattern",
            noun_plural: "patterns",
        },
        "Glob" | "Find" => Category {
            gerund: "finding",
            noun_singular: "file",
            noun_plural: "files",
        },
        "Edit" | "Write" | "Update" => Category {
            gerund: "editing",
            noun_singular: "file",
            noun_plural: "files",
        },
        "Bash" | "Run" => Category {
            gerund: "running",
            noun_singular: "command",
            noun_plural: "commands",
        },
        "List" | "LS" => Category {
            gerund: "listing",
            noun_singular: "directory",
            noun_plural: "directories",
        },
        "Task" | "Agent" => Category {
            gerund: "running",
            noun_singular: "agent",
            noun_plural: "agents",
        },
        _ => Category {
            gerund: "running",
            noun_singular: "tool",
            noun_plural: "tools",
        },
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn aggregate_phrase(calls: &[ActiveToolCall]) -> String {
    let mut counts: Vec<(Category, usize)> = Vec::new();

    for call in calls {
        let cat = category_for(&call.display_name);
        if let Some(entry) = counts.iter_mut().find(|(c, _)| c.gerund == cat.gerund) {
            entry.1 += 1;
        } else {
            counts.push((cat, 1));
        }
    }

    let segments: Vec<String> = counts
        .iter()
        .map(|(cat, count)| {
            let noun = if *count == 1 {
                cat.noun_singular
            } else {
                cat.noun_plural
            };
            format!("{} {} {}", cat.gerund, count, noun)
        })
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
