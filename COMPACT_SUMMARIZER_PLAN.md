# Implementation Plan for Summarizer Module (V1)

## Overview
This plan implements the conversation history compaction feature as described in COMPACTION.md, creating a reusable summarizer module and integrating it with a new `/compact` command.

## Implementation Tasks

1. **Create the Summarizer Module Structure**:
   * **File to Create**: `src/conversations/summarizer.rs`
   * **Description**: Create the summarizer module with the `MessageSummarizer` struct and implementation based on the design in COMPACTION.md. This will be the core logic for summarizing conversation messages using the LLM backend.
   * **Validation**: Add the new module to `src/conversations/mod.rs` and run `bash: cargo check`.

2. **Update the Conversations Module**:
   * **File to Modify**: `src/conversations/mod.rs`
   * **Description**: Add the new summarizer module to the module declarations and re-export the `MessageSummarizer` struct.
   * **Validation**: Run `bash: cargo check`.

3. **Implement Conversation Compaction Logic**:
   * **File to Modify**: `src/conversations/conversation.rs`
   * **Description**: Add the `compact_with_summary` method to the `Conversation` struct that will replace old messages with a summary while preserving recent messages and system context.
   * **Validation**: Run `bash: cargo check`.

4. **Create the Compact Command**:
   * **File to Create**: `src/commands/compact_command.rs`
   * **Description**: Implement the `/compact` command that allows users to manually trigger conversation compaction. This will use the `MessageSummarizer` to create a summary and then call the conversation's compaction method.
   * **Validation**: Run `bash: cargo check`.

5. **Register the Compact Command**:
   * **File to Modify**: `src/commands/mod.rs`
   * **Description**: Add the compact command module to the module declarations.
   * **Validation**: Run `bash: cargo check`.

6. **Register the Compact Command in the Registry**:
   * **File to Modify**: `src/commands/register.rs`
   * **Description**: Register the `CompactCommand` in the command registry so it's available to users.
   * **Validation**: Run `bash: cargo check`.

7. **Add Unit Tests for the Summarizer**:
   * **File to Modify**: `src/conversations/summarizer.rs`
   * **Description**: Add unit tests for the summarization functionality, testing both successful summarization and error handling.
   * **Validation**: Run `bash: cargo test`.

8. **Add Unit Tests for the Compact Command**:
   * **File to Create/Modify**: `src/commands/compact_command.rs`
   * **Description**: Add unit tests for the compact command, testing various scenarios including edge cases like conversations that are too short to compact.
   * **Validation**: Run `bash: cargo test`.

9. **Integration Test for the Full Compaction Flow**:
   * **File to Create**: `src/conversations/summarizer_integration_test.rs` (or add to existing tests)
   * **Description**: Create an integration test that verifies the complete compaction flow from command execution through summarization to conversation modification.
   * **Validation**: Run `bash: cargo test`.

10. **Documentation Update**:
    * **File to Modify**: Update relevant documentation files to reflect the new `/compact` command and its usage.
    * **Description**: Document the new `/compact` command, its arguments, and behavior.
    * **Validation**: Manual review of documentation.