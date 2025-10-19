# Refactor Event Loop to Use Trait-Based Input Handlers

## Goal

Refactor the TUI event loop to use a trait-based approach for input handlers instead of hardcoded pattern matching. This
will improve scalability and maintainability by allowing new input handlers to be registered dynamically.

## Current Architecture Issues

The current implementation in `src/tui/event_loop.rs` hardcodes the input handling logic with explicit pattern matching
on keys and direct calls to functions in `src/tui/input_handlers.rs`. This approach is not scalable as new input
handlers would require modifying the event loop directly.

## Desired Architecture

Implement a pub trait (similar to the example in `src/tui/input_handler.rs`) where different input handlers become
structs that implement that trait. The knowledge of which input handler handles which key should be contained within the
specific input handler. Register the input handlers instead of hardcoding them in the event loop.

## Implementation Plan

### 1. Define the InputHandler Trait

* **File to Modify**: `src/tui/input_handler.rs`, this is a just a recommendation, feel free to adjust as needed.
* **Description**: Enhance the existing `InputHandler` trait to include all necessary methods for handling different
  types of input events. Add methods for handling key events, paste events, and determining if the handler should
  process an event.
* **Validation**: Run `bash: cargo check`.

### 2. Create Base Input Handler Structs

* **Files to Create**:
    * `src/tui/handlers/mod.rs`
    * `src/tui/handlers/permission_handler.rs`
    * `src/tui/handlers/approval_handler.rs`
    * `src/tui/handlers/completion_handler.rs`
    * `src/tui/handlers/normal_handler.rs`
* **Description**: Create a new module for handlers and implement specific handler structs that implement the
  `InputHandler` trait for each type of input handling (permission, approval, completion, normal keys).
* **Validation**: Add the new module to `src/tui/mod.rs` and run `bash: cargo check`.

### 3. Refactor Existing Handler Functions

* **File to Modify**: `src/tui/input_handlers.rs`
* **Description**: Convert the existing handler functions into methods of the new handler structs. This involves moving
  the logic from functions to struct implementations while maintaining the same functionality.
* **Validation**: Run `bash: cargo check`.

### 4. Update Event Loop Context

* **File to Modify**: `src/tui/event_loop.rs`
* **Description**: Modify the `EventLoopContext` struct to include a collection of registered input handlers.
* **Validation**: Run `bash: cargo check`.

### 5. Refactor Event Loop to Use Handlers

* **File to Modify**: `src/tui/event_loop.rs`
* **Description**: Replace the hardcoded pattern matching with a dynamic dispatch system that iterates through
  registered handlers. Each handler will decide if it should process an event and handle it accordingly.
* **Validation**: Run `bash: cargo check`.

### 6. Update Module Registration

* **File to Modify**: `src/tui/mod.rs`
* **Description**: Update the module system to include the new handlers module and register the input handlers during
  initialization.
* **Validation**: Run `bash: cargo check`.

### 7. Integration Testing

* **Files to Modify**: None (manual check)
* **Description**: This is a placeholder for the Coder Agent to manually test all input handling functionality to ensure
  it works correctly after the refactoring.
* **Validation**: Run the application and test various input scenarios including permission dialogs, approval dialogs,
  completion, and normal key handling.
