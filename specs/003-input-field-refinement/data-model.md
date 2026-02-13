# Data Model: Input Field Refinement

**Branch**: `003-input-field-refinement` | **Date**: 2025-12-11

## Entities

### TextAttachment

Represents large pasted content stored temporarily and separately from the main input field.

**Attributes**:
- `id: usize` - Unique sequential identifier within session (1, 2, 3...)
- `content: String` - Full text content of the paste
- `size_chars: usize` - Character count (cached for display)
- `line_count: usize` - Number of lines (cached for display)
- `created_at: Instant` - Creation timestamp for metadata

**Constraints**:
- `size_chars > 200` - Only pastes exceeding threshold become attachments
- `size_chars <= 5_000_000` - 5MB limit (~1 byte/char UTF-8 assumption)
- `id` unique within session (enforced by sequential generation)
- `content` must be valid UTF-8 (enforced by Rust `String`)

**Relationships**:
- Referenced in `InputContent` by ID token `[pasted text-{id}]`
- Many-to-one: Multiple attachments can exist per input session
- Owned by `AppState.attachments: Vec<TextAttachment>`

**Lifecycle**:
1. **Created**: On paste event when `content.len() > 200`
2. **Persists**: In memory (`AppState`) until submission or deletion
3. **Deleted**:
   - Explicitly via user action (attachment management UI)
   - Automatically on input submission (all attachments cleared)
4. **Never serialized**: Ephemeral, session-scoped only

**State Transitions**:
```
┌─────────────┐  Paste >200 chars   ┌─────────────┐
│   (None)    │ ───────────────────> │   Active    │
└─────────────┘                      └─────────────┘
                                            │
                           ┌────────────────┴────────────────┐
                           │ User Delete                     │ Submit Input
                           v                                 v
                     ┌──────────┐                      ┌──────────┐
                     │ Deleted  │                      │ Deleted  │
                     └──────────┘                      └──────────┘
                     (removed from Vec)                (all cleared)
```

### InputContent

Represents the main text being composed by the user (existing entity, extended).

**Attributes** (existing):
- `text: String` - Content of the input (managed by `TextArea`)
- `cursor_position: (u16, u16)` - Cursor location (managed by `TextArea`)

**Attributes** (NEW):
- `mode: InputMode` - Current display/editing mode

**Relationships**:
- May reference zero or more `TextAttachment` via ID tokens in text
- Owned by `AppState.input: TextArea`

**Lifecycle**:
1. **Created**: On app startup (empty)
2. **Modified**: User types, pastes, edits
3. **Submitted**: Content (with expanded attachments) sent to LLM
4. **Cleared**: After submission, reset to empty

### InputMode (new enum)

Represents the current input editing mode.

**Values**:
- `Normal` - Standard input area at bottom of screen
- `Expanded` - Enlarged editor (50-60% terminal height)
- `AttachmentList` - Viewing list of attachments
- `AttachmentView` - Viewing/editing one specific attachment

**Relationships**:
- Owned by `AppState.input_mode: InputMode`
- Affects rendering logic in `Input` and `ExpandedEditor` components

**State Transitions**:
```
                    Ctrl+E
     ┌─────────────────────────────────────┐
     │                                     │
     v                                     │
┌─────────┐                           ┌──────────┐
│ Normal  │ <───────── Esc ─────────> │ Expanded │
└─────────┘                           └──────────┘
     │                                     │
     │ Ctrl+A                              │ Ctrl+A
     v                                     v
┌────────────────┐  Enter on item   ┌────────────────┐
│ AttachmentList │ ───────────────>  │ AttachmentView │
└────────────────┘                   └────────────────┘
     ^                                     │
     │                                     │
     └──────────────── Esc ────────────────┘
```

### AttachmentViewState (new struct)

Represents the state when viewing/editing a specific attachment.

**Attributes**:
- `attachment_id: usize` - ID of attachment being viewed
- `editor: TextArea` - Text editor for attachment content
- `is_modified: bool` - Whether content changed since opening

**Relationships**:
- References one `TextAttachment` by ID
- Owned by `AppState.attachment_view: Option<AttachmentViewState>`
- `None` when not viewing an attachment

**Lifecycle**:
1. **Created**: When user selects attachment to view
2. **Modified**: User edits content in editor
3. **Saved**: Content written back to `TextAttachment`, sizes recalculated
4. **Destroyed**: On Esc (discard changes) or Save (commit changes)

## Validation Rules

### Paste Classification

```rust
fn classify_paste(content: &str) -> PasteClassification {
    let size_bytes = content.len();

    if size_bytes > 5_000_000 {
        PasteClassification::Rejected("Paste rejected: exceeds 5MB limit".into())
    } else if content.chars().count() > 200 {
        PasteClassification::Attachment
    } else {
        PasteClassification::Inline
    }
}
```

**Rules**:
- Size check first (fail fast on huge pastes)
- Character count uses `.chars().count()` (not byte length, handles multi-byte UTF-8)
- Exactly 200 chars → Inline (≤ threshold)

### Attachment Reference Token

**Format**: `[pasted text-{id}]`

**Validation**:
- `id` must be numeric and exist in `attachments` vector
- Format is literal (no user customization)
- Token is plain text (not special object) in `TextArea.content`

### Content Expansion on Submit

```rust
fn expand_attachments(input: &str, attachments: &[TextAttachment]) -> String {
    let mut expanded = input.to_string();
    for attachment in attachments {
        let token = format!("[pasted text-{}]", attachment.id);
        expanded = expanded.replace(&token, &attachment.content);
    }
    expanded
}
```

**Rules**:
- All attachment references replaced with full content
- Replacement order doesn't matter (IDs are unique)
- After expansion, attachments cleared (single-use)

## Data Flow

### Paste → Attachment Creation

```
User Paste
    │
    v
┌──────────────────┐
│ Crossterm Event  │
│ Event::Paste(str)│
└──────────────────┘
    │
    v
┌──────────────────┐      >200 chars      ┌────────────────────┐
│ PasteDetector    │ ──────────────────>   │ AttachmentStore    │
│ .classify_paste()│                       │ .create(content)   │
└──────────────────┘                       └────────────────────┘
    │                                              │ Returns ID
    │ ≤200 chars                                   v
    v                                      ┌────────────────────┐
┌──────────────────┐                      │ Insert reference   │
│ Insert inline    │                      │ [pasted text-{id}] │
│ into TextArea    │                      │ into TextArea      │
└──────────────────┘                      └────────────────────┘
```

### Submit → Attachment Expansion

```
User Submit (Enter)
    │
    v
┌──────────────────┐
│ Get input text   │
│ from TextArea    │
└──────────────────┘
    │
    v
┌──────────────────┐
│ expand_attachments│
│ (replace tokens)  │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Send to LLM      │
│ backend          │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Clear attachments│
│ Clear input      │
└──────────────────┘
```

### Mode Switch → Rendering

```
User presses Ctrl+E
    │
    v
┌──────────────────┐
│ Set input_mode = │
│ Expanded         │
└──────────────────┘
    │
    v
┌──────────────────┐
│ Next frame render│
└──────────────────┘
    │
    v
┌──────────────────┐      input_mode?       ┌────────────────────┐
│ App render loop  │ ──── Expanded ───────>  │ ExpandedEditor     │
└──────────────────┘                         │ .render()          │
    │                                         └────────────────────┘
    │ Normal                                         │
    v                                                v
┌──────────────────┐                         ┌────────────────────┐
│ Input (normal)   │                         │ Render TextArea in │
│ .render()        │                         │ 50-60% height area │
└──────────────────┘                         └────────────────────┘
```
