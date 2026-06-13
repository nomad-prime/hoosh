use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn truncate_passes_short_content_unchanged() {
    let raw = "- [Alpha](alpha.md) — hook\n- [Beta](beta.md) — hook";
    let t = truncate_entrypoint(raw);
    assert!(!t.was_line_truncated);
    assert!(!t.was_byte_truncated);
    assert_eq!(t.line_count, 2);
    assert_eq!(t.content, raw);
}

#[test]
fn truncate_line_cap_fires_and_warns() {
    let raw = (0..300)
        .map(|i| format!("- [Mem{i}](m{i}.md) — hook"))
        .collect::<Vec<_>>()
        .join("\n");
    let t = truncate_entrypoint(&raw);
    assert!(t.was_line_truncated);
    assert_eq!(t.line_count, 300);
    assert!(t.content.contains("WARNING"));
    assert!(t.content.contains("limit: 200"));
}

#[test]
fn truncate_byte_cap_fires_when_lines_short() {
    let long_line = "x".repeat(30_000);
    let t = truncate_entrypoint(&long_line);
    assert!(t.was_byte_truncated);
    assert!(t.content.contains("index entries are too long"));
}

#[test]
fn truncate_handles_empty_input() {
    let t = truncate_entrypoint("");
    assert_eq!(t.line_count, 0);
    assert_eq!(t.byte_count, 0);
    assert!(t.content.is_empty());
}

#[test]
fn load_entrypoint_returns_none_when_file_missing() {
    let dir = TempDir::new().unwrap();
    assert!(load_entrypoint(dir.path()).is_none());
}

#[test]
fn load_entrypoint_reads_existing_file() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(ENTRYPOINT_NAME), "- [A](a.md) — hook\n").unwrap();
    let t = load_entrypoint(dir.path()).unwrap();
    assert_eq!(t.content, "- [A](a.md) — hook");
}

#[test]
fn build_memory_prompt_with_empty_index_states_so() {
    let dir = TempDir::new().unwrap();
    let block = build_memory_prompt(dir.path());
    assert!(block.contains("currently empty"));
    assert!(block.contains("save_memory"));
    assert!(block.contains(&dir.path().display().to_string()));
}

#[test]
fn build_memory_prompt_includes_existing_index() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(ENTRYPOINT_NAME),
        "- [User role](user.md) — data scientist\n",
    )
    .unwrap();
    let block = build_memory_prompt(dir.path());
    assert!(block.contains("data scientist"));
    assert!(!block.contains("currently empty"));
}
