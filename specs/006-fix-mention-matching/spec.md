# Feature Specification: Precise @mention Matching

**Feature Branch**: `006-fix-mention-matching`
**Created**: 2026-03-21
**Status**: Draft
**Input**: User description: "The mention check (body.contains(handle)) is a simple substring match — no word boundary check, so @hoosh-bot would also match @hoosh. We should have something more sophisticated: the handle should only match when it appears as a complete @mention (i.e. not as a prefix of a longer handle). For example, @hoosh should match when followed by whitespace, punctuation, or end of string — but not when followed by alphanumeric characters or hyphens (which are valid in GitHub usernames)."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configured Handle Matches Only Exact Mentions (Priority: P1)

A team configures hoosh with the handle `@hoosh`. When someone posts a GitHub comment containing `@hoosh`, the daemon responds. When someone posts a comment containing `@hoosh-bot` or `@hoosh2`, the daemon correctly ignores it because those are different accounts.

**Why this priority**: This is the core correctness bug. False positives cause the daemon to respond to mentions not directed at it, which is disruptive and potentially expensive (triggers an agent run).

**Independent Test**: Configure handle as `@hoosh`. Post a comment containing only `@hoosh-bot`. The daemon must not create a task. Post a comment containing `@hoosh`. The daemon must create a task.

**Acceptance Scenarios**:

1. **Given** the configured mention handle is `@hoosh`, **When** a comment body is `"Hey @hoosh-bot can you help?"`, **Then** no task is created.
2. **Given** the configured mention handle is `@hoosh`, **When** a comment body is `"Hey @hoosh can you help?"`, **Then** a task is created.
3. **Given** the configured mention handle is `@hoosh`, **When** a comment body is `"@hoosh2 please look at this"`, **Then** no task is created.
4. **Given** the configured mention handle is `@hoosh`, **When** a comment body is `"@hoosh please look at this"`, **Then** a task is created.

---

### User Story 2 - Handle Matches in Various Punctuation Contexts (Priority: P2)

A user mentions `@hoosh` in realistic comment prose: at the start of a sentence, in the middle, at the end, followed by a comma, period, exclamation mark, or colon. All of these must be recognized as valid mentions.

**Why this priority**: GitHub comments use natural language; a rigid match would miss many legitimate mentions and frustrate users.

**Independent Test**: Send comments with `@hoosh,`, `@hoosh.`, `@hoosh!`, `@hoosh:`, and `@hoosh` at end of string. All must trigger a task.

**Acceptance Scenarios**:

1. **Given** handle is `@hoosh`, **When** comment is `"@hoosh, please review"`, **Then** a task is created.
2. **Given** handle is `@hoosh`, **When** comment is `"Thanks @hoosh."`, **Then** a task is created.
3. **Given** handle is `@hoosh`, **When** comment is `"@hoosh"` (handle alone, end of string), **Then** a task is created.
4. **Given** handle is `@hoosh`, **When** comment is `"cc @hoosh!"`, **Then** a task is created.

---

### User Story 3 - Handle With Hyphens in the Configured Name Is Also Matched Precisely (Priority: P3)

A team uses a handle like `@hoosh-ci`. When a comment mentions `@hoosh-ci`, it matches. When it mentions `@hoosh-ci-staging`, it does not.

**Why this priority**: Handles may themselves contain hyphens (valid GitHub username characters), so the matching rule must be defined relative to the end of the configured handle, not just `@`.

**Independent Test**: Configure handle as `@hoosh-ci`. Post a comment with `@hoosh-ci-staging`. No task. Post with `@hoosh-ci`. Task created.

**Acceptance Scenarios**:

1. **Given** handle is `@hoosh-ci`, **When** comment contains `"@hoosh-ci-staging"`, **Then** no task is created.
2. **Given** handle is `@hoosh-ci`, **When** comment contains `"@hoosh-ci please run"`, **Then** a task is created.
3. **Given** handle is `@hoosh-ci`, **When** comment contains `"@hoosh-ci"` at end of string, **Then** a task is created.

---

### Edge Cases

- What happens when the mention handle appears multiple times in the same comment (e.g., `"@hoosh @hoosh-bot @hoosh please help"`)? The presence of at least one exact match should be sufficient to trigger.
- What happens when the comment body is empty or consists only of whitespace? No match; no task.
- What happens when the handle appears inside a code block (e.g., `` `@hoosh` ``) or a blockquote? Assumed to still match — filtering by markdown context is out of scope for this feature.
- What happens with Unicode punctuation following the handle (e.g., `@hoosh…`)? Assumed to match — Unicode non-alphanumeric, non-hyphen characters count as valid terminators.
- Case sensitivity: GitHub usernames are case-insensitive. Matching should be case-insensitive (e.g., `@Hoosh` matches handle `@hoosh`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mention detection logic MUST recognize a handle as mentioned only when the handle appears as a complete token — i.e., not immediately followed by an alphanumeric character or a hyphen.
- **FR-002**: The mention detection logic MUST match the handle when it is followed by whitespace, standard punctuation (`. , ! ? : ; ) ] }`), or the end of the string.
- **FR-003**: The mention detection logic MUST be case-insensitive to match GitHub's own username case-insensitivity.
- **FR-004**: The mention detection logic MUST correctly handle configured handles that themselves contain hyphens, applying the boundary check only after the full configured handle.
- **FR-005**: The existing `mentions_handle` function interface MUST remain compatible — callers pass a body string and a handle string and receive a boolean result; no caller changes are required.
- **FR-006**: The updated logic MUST have unit test coverage for: exact match, prefix-of-longer-handle, handle-with-hyphen configured, punctuation terminators, end-of-string, empty body, case variants.

### Assumptions

- Markdown context (code blocks, blockquotes) is not considered — the match applies to the raw comment body string.
- The `@` prefix is part of the configured handle string (e.g., `"@hoosh"`), consistent with current behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A comment containing `@hoosh-bot` does not trigger a task when the configured handle is `@hoosh` — verified by automated test with 100% pass rate.
- **SC-002**: All existing mention-match tests continue to pass without modification.
- **SC-003**: At least 8 distinct unit test cases (exact match, prefix variants, punctuation terminators, end-of-string, empty body, case variants, hyphenated handle) pass for the updated matching logic.
- **SC-004**: No regression in daemon behavior for any currently supported GitHub event type.
