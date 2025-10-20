# Hoosh Agent Switching - Complete Implementation Plan

## Overview

Implement agent switching functionality that allows users to change the active agent during a session using the
`/switch-agent` command.

---

## Implementation Steps

### **1. Add `current_agent_name` to `EventLoopContext`**

✅ **COMPLETED** - Added tracking for the currently active agent in the event loop context in `src/tui/event_loop.rs`.

---

### **2. Initialize `current_agent_name` in TUI Setup**

✅ **COMPLETED** - When creating the `EventLoopContext`, initialize it with the default agent name in `src/tui/mod.rs`.

---

### **3. Extend `CommandContext` to Include Current Agent Name**

✅ **COMPLETED** - Added `current_agent_name` field to `CommandContext` so commands can access it in `src/commands/registry.rs`.

---

### **4. Add `AgentSwitched` Event**

✅ **COMPLETED** - Added a new event type to signal when an agent has been switched in `src/conversations/handler.rs`.

---

### **5. Handle Agent Switch Event in Event Loop**

✅ **COMPLETED** - Handle the `AgentSwitched` event to update the context's current agent name in `src/tui/event_loop.rs`.

---

### **6. Update Command Execution to Pass Agent Name**

✅ **COMPLETED** - Pass the current agent name when creating `CommandContext` in `src/tui/actions.rs`.

---

### **7. Create the Switch Agent Command**

✅ **COMPLETED** - Implemented the `Command` trait for a new `SwitchAgentCommand` in `src/commands/switch_agent_command.rs`.

---

### **8. Add `get_agent` Method to AgentManager**

✅ **COMPLETED** - Added method to retrieve a specific agent by name in `src/agents/mod.rs`.

---

### **9. Add Event Sender to CommandContext**

✅ **COMPLETED** - Added event sender so commands can emit events in `src/commands/registry.rs`.

---

### **10. Pass Event Sender to CommandContext**

✅ **COMPLETED** - Pass event sender when creating CommandContext in `src/tui/actions.rs`.

---

### **11. Update SwitchAgentCommand to Send Event**

✅ **COMPLETED** - Send `AgentSwitched` event after successful switch in `src/commands/switch_agent_command.rs`.

---

### **12. Register the SwitchAgentCommand**

✅ **COMPLETED** - Register the new command with the command registry in `src/commands/register.rs`.

---

### **13. Update Agents Command to Show Current Agent**

✅ **COMPLETED** - Mark the current agent in the list in `src/commands/agents_command.rs`.

---

### **14. Update Status Command to Show Current Agent**

✅ **COMPLETED** - Display the currently active agent instead of just the default in `src/commands/status_command.rs`.

---

### **15. Update TUI Header to Reflect Current Agent**

✅ **COMPLETED** - The header already dynamically displays the current agent name, which is updated through the event system.

---

### **16. Final Integration Testing**

✅ **COMPLETED** - All functionality has been implemented and tested.

---

## Optional Enhancements (Future Work)

### **17. Implement Handoff Summary** (Optional)

* **Description**: When switching agents, generate a summary from the outgoing agent to provide context to the incoming
  agent.
* **Implementation**:
    1. Before switching, make an API call to the current agent asking for a summary
    2. Add the summary as a user message in the conversation
    3. Format: `[Agent Handoff from <old_agent> to <new_agent>]\n<summary>`
* **Benefits**: Better context retention without enlarging message history

### **18. Add `/current-agent` Command** (Optional)

* **File to Create**: `src/commands/current_agent_command.rs`
* **Description**: Simple command to show just the current agent
* **Implementation**: Similar to status command but only shows current agent

---

## Validation & Testing

After each step:

✅ Run `cargo check` to ensure no compilation errors
✅ Run `cargo build` to verify successful build
✅ Run `cargo test` to ensure all tests pass
✅ Manual testing completed successfully

---

## Notes

- The `Conversation` struct remains unchanged - it only stores messages
- Agent state lives in `EventLoopContext` as runtime state
- Commands receive current agent name through `CommandContext`
- Agent switching updates both the system message and the context state
- All existing conversation history is preserved when switching agents
- The implementation follows the project's coding style and conventions as outlined in `AGENTS.md`