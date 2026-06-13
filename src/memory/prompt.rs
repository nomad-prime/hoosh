use std::path::Path;

use crate::memory::entrypoint::{ENTRYPOINT_NAME, load_entrypoint};

const MEMORY_INSTRUCTIONS: &str = include_str!("../prompts/memory_instructions.txt");

pub fn build_memory_prompt(memory_root: &Path) -> String {
    let memory_root_display = memory_root.display().to_string();
    let instructions = MEMORY_INSTRUCTIONS.replace("{{memory_root}}", &memory_root_display);

    let index_block = match load_entrypoint(memory_root) {
        Some(t) if !t.content.is_empty() => format!("## {}\n\n{}", ENTRYPOINT_NAME, t.content),
        _ => format!(
            "## {}\n\nYour {} is currently empty. As you accumulate memories, this index will summarize them.",
            ENTRYPOINT_NAME, ENTRYPOINT_NAME
        ),
    };

    format!("# Memory\n\n{}\n\n{}", instructions, index_block)
}
