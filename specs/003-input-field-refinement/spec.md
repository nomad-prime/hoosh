# Feature Specification: Input Field Refinement

**Feature Branch**: `003-input-field-refinement`
**Created**: 2025-12-11
**Status**: Draft
**Input**: User description: "I want to refine the input field. currently copy pasting breaks the ui. specially copy pasting large text. we should also be able to enter text maybe in a bigger view(text editor). the input should also never extend beyond width of terminal, it should break line and become multi line when reaching the end. pasting large pieces of text should only be shown as [pasted text-#id] and kept like an attachement."

## Clarifications

### Session 2025-12-11

- Q: What metric should trigger attachment creation for pasted content (character count, line count, or both)? → A: Character count only
- Q: When should attachments be cleared, and how should they be serialized when submitted? → A: Cleared after input submission; attachment content expanded inline into message before storage
- Q: How should soft-wrapped lines be visually distinguished from hard line breaks? → A: Visual indicator symbol (e.g., "↩" or "⤶") at wrap points
- Q: How should very long unbreakable words (URLs, paths) that exceed terminal width be handled? → A: Force-break at terminal width boundary (visual only, content intact) with visual indicator
- Q: What is the maximum attachment size limit to prevent memory exhaustion? → A: 5MB per attachment; reject larger pastes with error message
- Q: What character count threshold should trigger attachment creation for pasted content? → A: 200 characters
- Q: Which hotkey should activate the expanded editor view? → A: Ctrl+E
- Q: Should the expanded editor be an internal TUI component or launch an external editor? → A: Internal TUI component using tui-textarea widget
- Q: How much screen space should the expanded editor occupy? → A: 50-60% of terminal height
- Q: Which keybinding should return from expanded view to normal input mode? → A: Esc

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Paste Large Content Without Breaking UI (Priority: P1)

A user copies a large code snippet, log output, or document from another application and pastes it into the input field. The interface remains stable and usable, with the large content represented as an attachment reference.

**Why this priority**: Core functionality that directly addresses the primary problem - pasting breaks the current UI. Without this, users experience broken interfaces and potential data loss.

**Independent Test**: Can be fully tested by pasting content exceeding 200 characters and verifying the UI remains stable, content is stored, and displays as `[pasted text-#id]`. Delivers immediate value by preventing UI breakage.

**Acceptance Scenarios**:

1. **Given** user copies 500 lines of code, **When** they paste into input field, **Then** UI displays `[pasted text-1]` reference and remains stable
2. **Given** user has pasted large text as attachment, **When** they submit the input, **Then** the full attachment content is expanded inline into the message and attachments are cleared
3. **Given** user pastes text of 200 characters or fewer, **When** paste completes, **Then** text appears directly in input field without attachment creation
4. **Given** user pastes content exceeding 5MB, **When** paste operation completes, **Then** system displays clear error message and rejects the paste

---

### User Story 2 - Text Wraps to Terminal Width (Priority: P1)

A user types or pastes text that approaches the terminal width. Instead of extending beyond the visible area, the text automatically wraps to the next line, maintaining readability and preventing horizontal overflow.

**Why this priority**: Critical for usability across different terminal sizes. Without wrapping, content becomes invisible or requires horizontal scrolling, making the application unusable on narrower terminals.

**Independent Test**: Can be tested by typing or pasting text until it reaches terminal edge, then verifying automatic wrapping occurs. Resize terminal to confirm dynamic rewrapping. Delivers value by ensuring all content remains visible.

**Acceptance Scenarios**:

1. **Given** terminal is 80 columns wide, **When** user types text reaching column 78, **Then** text wraps to next line automatically
2. **Given** multi-line wrapped text is displayed, **When** user resizes terminal to narrower width, **Then** text rewraps to fit new width
3. **Given** multi-line input with wrapping, **When** user navigates with arrow keys, **Then** cursor moves correctly through wrapped and hard-break lines
4. **Given** text has wrapped lines and hard line breaks, **When** displayed, **Then** soft-wrap points show visual indicator symbols while hard breaks do not

---

### User Story 3 - Edit in Expanded View (Priority: P2)

A user needs to compose or edit a lengthy, complex message. They activate an internal expanded editor view (built using tui-textarea widget) that provides more screen space within the hoosh interface, making it easier to review and modify their content before submitting.

**Why this priority**: Enhances user experience for complex tasks but not critical for basic functionality. Users can still input text without it, but extended editing is uncomfortable in single-line mode.

**Independent Test**: Can be tested by activating expanded view, entering multi-line content, editing it, and returning to normal mode. Delivers value by improving comfort for longer compositions.

**Acceptance Scenarios**:

1. **Given** user is in normal input mode, **When** they press Ctrl+E, **Then** interface switches to expanded editor view
2. **Given** user is in expanded editor view with content, **When** they press Esc to return to normal mode, **Then** all content is preserved
3. **Given** expanded editor contains 50 lines, **When** user scrolls, **Then** they can view all content smoothly
4. **Given** user is in expanded view, **When** interface renders, **Then** editor occupies 50-60% of terminal height

---

### User Story 4 - Manage Attached Content (Priority: P3)

A user has pasted multiple large texts as attachments and wants to review, edit, or remove them before submitting. They can view a list of attachments, open any attachment to see its content, make edits, or delete unwanted items.

**Why this priority**: Important for complete workflow but less critical than core paste handling. Users can work without this if they're careful with their pastes, but it significantly improves confidence and control.

**Independent Test**: Can be tested by creating multiple attachments, listing them, viewing/editing individual attachments, and deleting one. Delivers value by providing full control over attached content.

**Acceptance Scenarios**:

1. **Given** user has created two attachments, **When** they request attachment list, **Then** both attachments are shown with identifiers and metadata (size, line count)
2. **Given** attachment list is displayed, **When** user selects an attachment to view, **Then** full content is displayed
3. **Given** user is viewing attachment content, **When** they make edits and press Ctrl+S to save, **Then** changes are preserved in the attachment and metadata (size, line count) is recalculated
4. **Given** user has an unwanted attachment, **When** they delete it, **Then** attachment is removed and reference disappears from input

---

### Edge Cases

- What happens when user pastes exactly 200 characters (at threshold boundary)? (Recommendation: treat as inline paste since it's ≤ threshold)
- How does system handle paste operations when terminal width is extremely narrow (e.g., 40 columns)?
- What happens if user pastes binary data or non-text content?
- Pastes exceeding 5MB are rejected with a clear error message to prevent memory exhaustion
- What happens when user tries to edit an attachment reference token in the input field?
- Very long words (URLs, file paths) exceeding terminal width are force-broken visually at the boundary with the "↩" indicator (same as soft-wraps); actual content remains intact
- What happens if user resizes terminal during paste operation?
- How does system handle Unicode characters, emojis, or wide characters in wrapping calculations?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect when pasted content exceeds 200 characters
- **FR-002**: System MUST reject pastes exceeding 5MB with a clear error message
- **FR-003**: System MUST store large pasted content as attachments with unique identifiers
- **FR-004**: System MUST display attachment references in the format `[pasted text-#id]` in the input field
- **FR-005**: System MUST wrap text automatically when approaching terminal width boundary
- **FR-006**: System MUST rewrap text dynamically when terminal is resized
- **FR-007**: System MUST display the visual indicator symbol "↩" (U+21A9 LEFTWARDS ARROW WITH HOOK) at soft-wrap points to distinguish them from hard line breaks
- **FR-008**: Users MUST be able to activate an expanded editor view via Ctrl+E hotkey
- **FR-009**: Expanded editor view MUST be an internal TUI component (using tui-textarea widget) that occupies 50-60% of terminal height
- **FR-010**: Users MUST be able to return from expanded view to normal mode via Esc key while preserving content
- **FR-011**: System MUST provide scrolling capability in expanded editor view for content exceeding visible area
- **FR-022**: System MUST NOT launch external editors (all editing happens within the hoosh TUI interface)
- **FR-012**: Users MUST be able to view a list of all current attachments with metadata
- **FR-013**: Users MUST be able to view full content of any attachment
- **FR-014**: Users MUST be able to edit attachment content
- **FR-015**: Users MUST be able to delete attachments
- **FR-016**: System MUST expand attachment content inline into the message when user submits input (attachment references replaced with full content before storage)
- **FR-017**: System MUST clear all attachments after input is successfully submitted
- **FR-018**: System MUST insert pastes of 200 characters or fewer directly into input field
- **FR-019**: System MUST handle word boundaries intelligently during wrapping (avoid breaking words mid-character), except when a single word exceeds terminal width, in which case it MUST force-break the word at the boundary with the same visual indicator "↩" as soft-wraps (content remains intact, break is display-only)
- **FR-020**: System MUST maintain correct cursor navigation across wrapped lines and hard breaks
- **FR-021**: Input field MUST never extend horizontally beyond terminal width

### Key Entities

- **Text Attachment**: Represents large pasted content stored temporarily and separately from the main input field (ephemeral, session-scoped)
  - Attributes: unique identifier, content text, size in characters, line count, creation timestamp
  - Constraints: Maximum 5MB per attachment; pastes exceeding this limit are rejected with error
  - Relationships: Referenced in input field by ID token; content expanded inline when submitted; cleared after successful submission
  - Lifecycle: Created on paste exceeding threshold; persists until submission or explicit deletion; not saved across sessions

- **Input Content**: The main text being composed by the user
  - Attributes: text content, cursor position, display mode (normal/expanded)
  - Relationships: May contain zero or more attachment references

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can paste content of 10,000+ characters without UI layout breaking or content loss
- **SC-002**: Text automatically wraps at terminal boundaries for terminals ranging from 80 to 240 columns width
- **SC-003**: Users can compose and edit messages exceeding 100 lines comfortably in expanded view
- **SC-004**: 95% of paste operations complete without requiring user intervention or correction
- **SC-005**: Terminal resize operations trigger rewrapping within 100 milliseconds
- **SC-006**: Users can navigate with cursor through 50+ lines of mixed wrapped and hard-break content without confusion
- **SC-007**: Attachment management (list, view, edit, delete) can be completed in under 30 seconds per attachment
