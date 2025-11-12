# Ticket: Implement Markdown Rendering with Syntax Highlighting for LLM Responses

## Problem Statement

Currently, LLM responses are rendered as plain text in the TUI. The application needs rich markdown rendering with
syntax highlighting to match the visual quality of Claude Code, making code blocks, formatting, and structured content
more readable and professional.

## Current State

- `MessageRenderer` in `message_renderer.rs` renders all LLM responses as plain text
- No markdown parsing or syntax highlighting for code blocks
- No differentiation between different markdown elements (headers, lists, code, etc.)
- Tool outputs and responses lack visual structure

## Requirements

### Core Markdown Features

1. **Code blocks with syntax highlighting**
    - Detect language from fence markers (```rust, ```python, etc.)
    - Syntax highlight based on language
    - Line numbers for code blocks
    - Background color differentiation

2. **Inline code formatting**
    - Monospace font with distinct background
    - Different color from regular text

3. **Text formatting**
    - Bold text (`**bold**`)
    - Italic text (`*italic*`)
    - Strikethrough (`~~strikethrough~~`)

4. **Structural elements**
    - Headers (h1-h6) with size/weight differentiation
    - Lists (ordered and unordered)
    - Block quotes with left border
    - Horizontal rules

5. **Links**
    - Distinguish link text from URLs
    - Consider future clickability

### Implementation Approach

**Phase 1: Markdown Parsing**

- Add `pulldown-cmark` dependency for markdown parsing
- Create `MarkdownRenderer` component in `tui/markdown.rs`
- Parse markdown into AST structure

**Phase 2: Syntax Highlighting**

- Add `syntect` crate for syntax highlighting
- Support common languages: Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Shell, SQL, JSON, YAML, TOML
- Fallback to plain text for unsupported languages
- Load appropriate theme (consider dark theme compatibility)

**Phase 3: TUI Rendering**

- Convert markdown AST to styled `ratatui` spans/lines
- Apply colors from theme configuration
- Handle line wrapping for long code blocks
- Maintain scrollability with formatted content

**Phase 4: Integration**

- Update `MessageRenderer` to use `MarkdownRenderer` for assistant responses
- Ensure streaming responses update incrementally
- Handle partial markdown during streaming (incomplete code blocks, etc.)

### Technical Design

```rust
// tui/markdown.rs
pub struct MarkdownRenderer {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        // Initialize syntax highlighting
    }

    pub fn render(&self, markdown: &str) -> Vec<Line<'static>> {
        // Parse markdown and convert to styled lines
    }

    fn render_code_block(&self, lang: &str, code: &str) -> Vec<Line<'static>> {
        // Syntax highlight code block
    }

    fn render_inline(&self, events: &[Event]) -> Vec<Span<'static>> {
        // Handle inline formatting
    }
}
```

### Dependencies to Add

```toml
[dependencies]
pulldown-cmark = "0.11"  # Markdown parsing
syntect = "5.2"           # Syntax highlighting
```

### Considerations

**Performance**

- Syntax highlighting can be CPU-intensive for large code blocks
- Consider caching highlighted results for unchanged content
- May need background task for highlighting during streaming

**Testing Strategy**

- Unit tests for markdown parsing edge cases
- Visual tests for various markdown elements
- Streaming scenarios with partial content
- Long code blocks and scrolling behavior

### Success Criteria

- [ ] Code blocks render with syntax highlighting in appropriate colors
- [ ] Headers, lists, and formatting render distinctly
- [ ] Streaming responses update smoothly with markdown rendering
- [ ] Performance remains acceptable for long responses
- [ ] Appearance matches or exceeds Claude Code quality
- [ ] Configuration allows theme customization

### Files to Modify/Create

- **Create**: `src/tui/markdown.rs` - Core markdown rendering logic
- **Modify**: `src/tui/message_renderer.rs` - Integrate markdown renderer
- **Modify**: `src/config/mod.rs` - Add markdown theme configuration
- **Modify**: `Cargo.toml` - Add dependencies
- **Create**: `tests/markdown_rendering.rs` - Test suite
