# Quickstart: Refined Input Field

**Branch**: `003-input-field-refinement` | **Date**: 2025-12-11

## Overview

The hoosh input field now supports:
- **Large paste handling**: Paste 10,000+ characters without breaking the UI
- **Automatic text wrapping**: Content never extends beyond terminal width
- **Expanded editor mode**: 50-60% screen editor for comfortable multi-line editing
- **Attachment management**: Review, edit, or delete pasted content before submitting

## Basic Usage

### Normal Input

Type normally at the prompt:
```
> Hello, how are you?
```

Press **Enter** to submit.

### Large Paste Handling

When you paste text **over 200 characters**:
1. Content is automatically saved as an attachment
2. A reference token appears in the input: `[pasted text-1]`
3. The UI remains stable and responsive

Example:
```
> I have a question about [pasted text-1]
```

**Note**: When you submit, the full attachment content is automatically expanded inline.

### Text Wrapping

Text automatically wraps at terminal edges:
```
> This is a very long line that will automatically wrap when it reaches
↩ the edge of your terminal window without extending beyond the visible
↩ area
```

The **↩** symbol indicates a soft-wrap (automatic). Hard line breaks (when you press Enter while typing) have no indicator.

### Expanded Editor Mode

For comfortable multi-line editing:

1. Press **Ctrl+E** to open expanded editor
2. Editor occupies 50-60% of screen height
3. Type, edit, navigate normally
4. Press **Esc** to return to normal mode

All content is preserved when switching modes.

```
╭─ Expanded Editor (Esc to exit) ─────────────────╮
│                                                  │
│ This is a longer message that I'm composing     │
│ across multiple lines in the expanded view.     │
│                                                  │
│ I can see more context here and edit           │
│ comfortably before submitting.                   │
│                                                  │
│                                                  │
╰──────────────────────────────────────────────────╯
```

### Attachment Management

To review, edit, or delete attachments:

1. Press **Ctrl+A** to open attachment list
2. Use **↑/↓** to select an attachment
3. Press **Enter** to view/edit the attachment
4. Press **d** to delete the selected attachment
5. Press **Esc** to close and return

```
╭─ Attachments (2) ────────────────────────────────╮
│ [1] pasted text-1  (1,234 chars, 45 lines)       │
│ [2] pasted text-2  (5,678 chars, 123 lines)      │
│                                                   │
│ Enter: View/Edit  d: Delete  Esc: Close          │
╰───────────────────────────────────────────────────╯
```

When viewing an attachment:
- Edit content directly
- Press **Ctrl+S** to save changes (required to persist edits)
- Press **Esc** to discard and return

## Edge Cases

### Paste Size Limits

- **≤200 chars**: Inserted directly into input (no attachment)
- **>200 chars, ≤5MB**: Saved as attachment
- **>5MB**: Rejected with error message

### Exactly 200 Characters

Pastes of exactly 200 characters are treated as inline (no attachment created).

### Terminal Resizing

Text automatically rewraps when you resize the terminal window. Wrapping recalculates within 100ms.

### Very Long Words

URLs or file paths exceeding terminal width are force-broken at the boundary:
```
> Check this URL: https://example.com/very/long/path/that/exceeds/termi
↩ nal/width/and/gets/broken/visually
```

The actual content remains intact (no characters lost), only the display is broken.

## Keyboard Shortcuts

| Key          | Action                              |
|--------------|-------------------------------------|
| **Ctrl+E**   | Open expanded editor                |
| **Esc**      | Exit expanded editor / attachment UI|
| **Ctrl+A**   | Open attachment list                |
| **Enter**    | Submit input / Select attachment    |
| **↑/↓**      | Navigate attachment list            |
| **d**        | Delete selected attachment          |
| **Ctrl+S**   | Save attachment edits               |

## Tips

1. **Use expanded editor for long messages**: Press Ctrl+E when composing multi-paragraph content
2. **Review large pastes before submitting**: Press Ctrl+A to check attachment content
3. **Clean up attachments**: Delete unwanted attachments with 'd' to keep input clean
4. **Terminal width matters**: For best wrapping, use at least 80 columns width
