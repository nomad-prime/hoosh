# Implementation Plan: Model Cascade System

**Branch**: `002-model-cascade` | **Date**: 2025-12-10 | **Spec**: `/specs/002-model-cascade/spec.md`

**Note**: This plan is filled in by the `/speckit.plan` command following the `/speckit.clarify` session.

## Summary

Implement a conservative, human-validated model cascade system that automatically selects appropriate model tiers (Light/Medium/Heavy) based on multi-signal task complexity analysis. Phase 1 defaults to Medium-tier for ambiguous cases and provides an `escalate` tool for agent-initiated escalations. All escalations require human-in-the-loop (HITL) approval before model switching occurs. The system is OFF by default (config-driven activation) and emits structured JSON events for monitoring and analysis.

## Technical Context

**Language/Version**: Rust 1.75+  
**Primary Dependencies**: tokio (async runtime), serde_json (event serialization), existing LLM backends (Anthropic, OpenAI, TogetherAI, Ollama)  
**Storage**: N/A (in-memory CascadeContext per request; no persistence in Phase 1)  
**Testing**: cargo test (unit and integration tests with human-labeled test dataset for accuracy validation)  
**Target Platform**: Same as Hoosh main (CLI agent on Linux/macOS)  
**Project Type**: Single Rust project (integrated into existing Hoosh codebase)  
**Performance Goals**: Escalation latency < 2 seconds (excluding LLM response time); message preservation 100%; routing accuracy 85%+ on complex tasks  
**Constraints**: Sequential execution only (no concurrency); HITL required before escalation; cascades disabled by default  
**Scale/Scope**: Phase 1 addresses single-backend escalation (e.g., all Anthropic or all OpenAI); 50-100 task dataset for validation

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify compliance with `.specify/memory/constitution.md`:

- [x] **Modularity First**: Feature creates new `cascades/` module with clear separation: complexity analysis (complexity_analyzer.rs), routing logic (router.rs), context management (context.rs), HITL validation (approval_handler.rs), and event emission (events.rs)
- [x] **Explicit Error Handling**: All cascade operations return `anyhow::Result<T>` with contextual error information (task_id, escalation path, reason)
- [x] **Async-First Architecture**: Escalation trigger is async; HITL approval UI is async; event logging is async; no blocking operations in cascade hot path
- [x] **Testing Discipline**: Tests focus on behavior (correct tier routing, context preservation, escalation flow) not implementation; test names: `routes_simple_task_to_light_tier`, `preserves_conversation_during_escalation`, `requires_hitl_approval`
- [x] **Simplicity and Clarity**: Cascade module uses explicit state machines (not generic traits); clear naming (ComplexityAnalyzer, EscalationRequest); minimal dependencies (serde_json, tokio)

**Violations**: None. Feature aligns with all core principles.

## Project Structure

### Documentation (this feature)

```text
specs/002-model-cascade/
├── spec.md              # Feature specification (with clarifications)
├── plan.md              # This file (implementation plan)
├── research.md          # Phase 0 output (research findings on complexity analysis techniques)
├── data-model.md        # Phase 1 output (entity definitions and relationships)
├── quickstart.md        # Phase 1 output (implementation walkthrough and integration guide)
├── contracts/           # Phase 1 output (API contracts for cascades module)
│   ├── complexity-analysis.yaml
│   ├── escalation-request.yaml
│   ├── cascade-config.yaml
│   └── cascade-events.yaml
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── cascades/                    # NEW: Model cascade system module
│   ├── mod.rs                   # Module declaration and public API
│   ├── complexity_analyzer.rs   # Multi-signal complexity analysis (structural depth, action density, code signals, concept count)
│   ├── router.rs                # Complexity-to-tier routing with conservative defaults
│   ├── context.rs               # CascadeContext management (in-memory per request)
│   ├── approval_handler.rs      # HITL (human-in-the-loop) approval workflow
│   ├── events.rs                # Structured JSON event emission for observability
│   ├── errors.rs                # Cascade-specific error types
│   └── tests.rs                 # Integration and behavior tests with human-labeled dataset
│
├── backends/
│   ├── mod.rs                   # (existing)
│   ├── strategy.rs              # MODIFY: Add tier selection support
│   └── [existing backend files]
│
├── agent/
│   ├── core.rs                  # MODIFY: Integrate cascade context into task execution
│   ├── conversation.rs          # MODIFY: Preserve context through escalation
│   └── [existing agent files]
│
├── tools/
│   └── mod.rs                   # MODIFY: Register new 'escalate' tool
│
├── config/
│   └── mod.rs                   # MODIFY: Support `cascades` config section parsing
│
└── [existing modules unchanged]

tests/
├── integration/
│   └── cascade_integration_test.rs   # End-to-end cascade flows with mock datasets
└── [existing test structure]
```

**Structure Decision**: Cascade system is implemented as a new `src/cascades/` module following Hoosh's existing modular structure. The module is self-contained with clear boundaries (complexity analysis, routing, context, approval, events) and integrates with existing backends, agent core, and tools. No existing code is refactored; integration points are clearly marked for modification.

## Complexity Tracking

No Constitution violations. Feature fully complies with all core principles.

---

## Phase 0: Research & Unknowns

### Research Tasks (to be resolved)

1. **Complexity Analysis Heuristics** - Research multi-signal complexity analysis techniques for code/task analysis
   - Task: Find established heuristics for structural depth, action density, code signals, and concept counting
   - Outcome: Define concrete metrics, thresholds, and confidence calculation formulas

2. **Human-in-the-Loop (HITL) UI Patterns** - Research best practices for escalation approval workflows in agent systems
   - Task: Identify UI patterns for presenting escalation decisions to operators (approval/rejection with context)
   - Outcome: Define UI contract and operator experience flow

3. **Structured Event Logging** - Research Rust patterns for structured JSON event logging and querying
   - Task: Identify efficient, queryable event logging strategies (in-memory index, eventual file persistence)
   - Outcome: Define event schema and query interface

### Research Dependencies

- **tokio ecosystem**: Confirm async patterns for context passing and tool invocation
- **serde_json**: Confirm serialization strategy for conversation history preservation
- **Observability**: Confirm metrics tracking patterns compatible with existing Hoosh monitoring

---

## Phase 1: Design & Contracts

### Design Artifacts

1. **data-model.md**: Entity definitions for TaskComplexity, ModelTier, CascadeContext, CascadeConfig
2. **contracts/**: API contracts for complexity analysis, escalation request/response, events schema
3. **quickstart.md**: Step-by-step integration guide showing how cascade module plugs into agent loop
4. **complexity_analysis.md**: Detailed heuristics and thresholds for multi-signal analysis

### Integration Points (marked for modification)

- **backends/strategy.rs**: Add tier selection support (map complexity level to backend-specific models)
- **agent/core.rs**: Inject CascadeContext into task execution, route task through complexity analyzer
- **agent/conversation.rs**: Ensure conversation history is thread-safe during escalation
- **tools/mod.rs**: Register `escalate` tool with HITL approval handler
- **config/mod.rs**: Parse `cascades` configuration section

---

## Phase 2 (Future)

Out of scope for this plan. Covers:
- Automatic downgrade/optimization post-escalation
- Cross-backend escalation
- Cost tracking and attribution
- User-triggered re-escalation
- Task completion with "unsolvable" outcome handling

---

## Gate Evaluation

**Gate Status**: ✅ **PASS**
- Technical context defined
- Constitution check passed
- No blocking unknowns (research tasks are straightforward)
- Project structure aligns with modular principles
- Integration points clearly identified

**Recommended Next Steps**:
1. Execute research tasks (Phase 0) using research agents
2. Consolidate findings in `research.md`
3. Generate data model and contracts (Phase 1)
4. Implement cascade module with tests
