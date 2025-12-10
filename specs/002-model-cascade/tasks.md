# Tasks: Model Cascade System

**Input**: Design documents from `/specs/002-model-cascade/`  
**Feature Branch**: `002-model-cascade`  
**Status**: Ready for implementation  

## Overview

- **Total Tasks**: 33
- **Setup & Foundational**: 7 tasks
- **User Stories**: 5 stories, 26 tasks
- **Parallel Opportunities**: 12 tasks marked [P]
- **MVP Scope**: US1 + US2 + US3 (core cascade features)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization

- [ ] T001 Create `src/cascades/` module directory structure
- [ ] T002 Create `src/cascades/mod.rs` with module declarations
- [ ] T003 [P] Create `src/cascades/errors.rs` with CascadeError types
- [ ] T004 [P] Create `src/cascades/types.rs` with TaskComplexity, ExecutionTier, ComplexityLevel

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure for all stories

- [ ] T005 Create `src/cascades/complexity_analyzer.rs` implementing ComplexityAnalyzer trait with multi-signal analysis (structural depth, action density, code signals, concept count)
- [ ] T006 Create `src/cascades/router.rs` implementing CascadeRouter trait with Light/Medium/Heavy routing logic and conservative Medium default
- [ ] T007 MODIFY `src/config/mod.rs` to parse `cascades` configuration section from .hoosh.toml

**Checkpoint**: Foundation ready - complexity analysis and routing operational

---

## Phase 3: User Story 1 - Automatic Model Selection Based on Task Complexity (Priority: P1) 🎯 MVP

**Goal**: System automatically analyzes task and routes to appropriate model tier

**Independent Test**: Submit simple/moderate/complex tasks and verify correct tier selection

### Implementation for US1

- [ ] T008 [P] [US1] Create `src/cascades/context.rs` with CascadeContext struct managing escalation state
- [ ] T009 [US1] MODIFY `src/agent/core.rs` to create CascadeContext at task start
- [ ] T010 [US1] MODIFY `src/agent/core.rs` to route task through ComplexityAnalyzer before model selection
- [ ] T011 [P] [US1] Create unit tests in `src/cascades/tests.rs` for routing logic with light/medium/heavy tasks
- [ ] T012 [US1] MODIFY `src/backends/strategy.rs` to map ExecutionTier to concrete model from CascadeConfig

**Checkpoint**: US1 complete - automatic model selection working with Medium default

---

## Phase 4: User Story 2 - Manual Escalation with Escalate Tool (Priority: P1)

**Goal**: Agent can invoke `escalate` tool to upgrade to next tier

**Independent Test**: Execute Light-tier task, trigger escalation, verify Medium-tier execution with preserved history

### Implementation for US2

- [ ] T013 [P] [US2] Create `src/cascades/escalate_tool.rs` implementing Tool trait for escalation
- [ ] T014 [US2] MODIFY `src/tools/mod.rs` to register escalate tool when cascades enabled
- [ ] T015 [US2] MODIFY `src/agent/conversation.rs` to ensure ChatMessage history is thread-safe for escalation
- [ ] T016 [P] [US2] Create unit tests for escalate tool invocation and tier progression
- [ ] T017 [US2] Implement escalation context switching to preserve full conversation history

**Checkpoint**: US2 complete - escalation workflow operational with context preservation

---

## Phase 5: User Story 3 - Cascade Configuration & Safe Defaults (Priority: P1)

**Goal**: Cascades OFF by default; only enable when config present

**Independent Test**: Verify cascades disabled without config, enabled with config

### Implementation for US3

- [ ] T018 [P] [US3] Create `src/cascades/config.rs` with CascadeConfig struct (Enabled, RoutingPolicy, DefaultTier, ModelTiers)
- [ ] T019 [US3] MODIFY `src/config/mod.rs` to load cascades section with safe defaults (enabled=false if absent)
- [ ] T020 [US3] Add validation in CascadeConfig: cascades disabled if no config section, enabled if section present
- [ ] T021 [P] [US3] Create unit tests for config loading (with/without cascades section)
- [ ] T022 [US3] MODIFY tool registration logic to conditionally register escalate tool based on `cascades.enabled`

**Checkpoint**: US3 complete - config-driven activation working, cascades OFF by default

---

## Phase 6: User Story 4 - Conservative Routing with Medium Default (Priority: P1)

**Goal**: Default to Medium-tier for ambiguous tasks (confidence < 0.7)

**Independent Test**: Route ambiguous tasks and verify all select Medium-tier

### Implementation for US4

- [ ] T023 [P] [US4] Implement confidence threshold logic in router.rs (confidence < 0.7 → Medium)
- [ ] T024 [US4] Create `src/cascades/routing_tests.rs` with human-labeled dataset of 50 tasks and tier expectations
- [ ] T025 [P] [US4] Add ComplexityMetrics calculation to analyzer for all four signals
- [ ] T026 [US4] Validate routing accuracy 85%+ on test dataset, multi-signal 15% better than length-only

**Checkpoint**: US4 complete - conservative Medium default working with multi-signal analysis

---

## Phase 7: User Story 5 - Preserve Conversation Context During Escalation (Priority: P2)

**Goal**: All prior messages preserved when escalating between tiers

**Independent Test**: Escalate mid-task, verify all prior conversation visible to new tier

### Implementation for US5

- [ ] T027 [P] [US5] Create conversation preservation logic in `src/cascades/context.rs`
- [ ] T028 [US5] MODIFY escalation handler to pass full ChatMessage history to new tier model
- [ ] T029 [P] [US5] Create integration tests verifying 100% message preservation through Light→Medium→Heavy escalations
- [ ] T030 [US5] Add escalation trace tracking to CascadeEvent showing tier progression

**Checkpoint**: US5 complete - context preservation validated across escalation boundaries

---

## Phase 8: Observability & Events

**Purpose**: Monitoring and debugging support

- [ ] T031 [P] Create `src/cascades/events.rs` implementing CascadeEventLogger trait with JSON event emission
- [ ] T032 [P] Add JSONL event persistence to `./cascade-events.jsonl`
- [ ] T033 Create integration test verifying event logs for create/route/escalate/complete lifecycle

**Checkpoint**: Observability infrastructure operational

---

## Dependencies & Execution Order

### Critical Path
1. **Setup (Phase 1)** → Foundational (Phase 2) → US1/2/3/4 (Phases 3-6) → US5 (Phase 7) → Observability (Phase 8)

### Parallel Opportunities After Foundational Complete
- **T008, T011 (US1 tests/context)** can run in parallel
- **T013, T016 (US2 tool/tests)** can run in parallel
- **T018, T021 (US3 config/tests)** can run in parallel
- **T023, T025 (US4 routing/metrics)** can run in parallel
- **T027, T029 (US5 preservation/tests)** can run in parallel
- **T031, T032 (Events)** can run in parallel

### User Story Independence
- **US1** (Model Selection): Standalone, no dependencies on other stories
- **US2** (Escalation): Depends on US1 infrastructure (routing, context)
- **US3** (Configuration): Depends on core infrastructure, enables US1/US2 activation
- **US4** (Conservative Default): Depends on US1 routing
- **US5** (Context Preservation): Depends on US2 escalation

---

## Implementation Strategy

### MVP Scope (Minimum Viable Product)
1. Complete Phase 1 Setup
2. Complete Phase 2 Foundational
3. Complete US1 + US3 (auto-routing + config control)
4. **VALIDATE**: Tasks with default Medium routing, no escalation tool yet

### Phase 1b (MVP+Escalation)
5. Complete US2 (escalate tool with HITL approval)
6. **VALIDATE**: Escalation workflow working

### Phase 2 (Full Feature)
7. Complete US4 (multi-signal accuracy validation)
8. Complete US5 (context preservation validation)
9. Complete Observability
10. **VALIDATE**: All features working, metrics tracked

---

## Testing Strategy

- **Unit tests**: Routing logic, tier assignment, config parsing (T011, T016, T021, T024)
- **Integration tests**: End-to-end escalation, context preservation (T029, T033)
- **Dataset**: 50-100 human-curated tasks with ground truth tier labels (T024)
- **Validation criteria**: 85%+ routing accuracy, 100% context preservation, <2s escalation latency

---

## Success Criteria Checklist

- [ ] SC-001: Routing 85%+ accurate on test dataset
- [ ] SC-005: 100% conversation history preservation through escalations
- [ ] SC-006: Escalation latency < 2 seconds
- [ ] SC-009: Cascades disabled by default (no config = standard mode)
- [ ] SC-010: Cascades enable correctly when config added
- [ ] All user stories independently testable
- [ ] HITL approval required for all escalations (Phase 1)

---

## Notes

- All tasks use exact file paths for clarity
- [P] indicates parallelizable tasks (different files, no dependencies)
- MODIFY tasks update existing files; specify exact locations
- Tests are required for routing accuracy and context preservation validation
- Cascades disabled by default unless `cascades` section in config
- HITL approval mandatory before escalation proceeds (US2 requirement)
