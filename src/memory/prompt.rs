use std::path::Path;

use crate::memory::entrypoint::{ENTRYPOINT_NAME, load_entrypoint};

pub fn build_memory_prompt(memory_root: &Path) -> String {
    let memory_root_display = memory_root.display();
    let entry = load_entrypoint(memory_root);

    let index_block = match entry {
        Some(t) if !t.content.is_empty() => format!("## {}\n\n{}", ENTRYPOINT_NAME, t.content),
        _ => format!(
            "## {}\n\nYour {} is currently empty. As you accumulate memories, this index will summarize them.",
            ENTRYPOINT_NAME, ENTRYPOINT_NAME
        ),
    };

    format!(
        "# Memory\n\nYou have a persistent, file-based memory at `{}`. This directory already exists — write to it directly with `save_memory` (do not run mkdir or check for its existence).\n\nMemory files are markdown with YAML frontmatter (`name`, `description`, `type`). The index below names what is currently saved; read individual files on demand with `read_file`.\n\n{}",
        memory_root_display, index_block
    )
}
