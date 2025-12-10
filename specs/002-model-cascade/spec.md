# Feature Specification: Model Cascade System

**Feature Branch**: `002-model-cascade`
**Created**: 2025-12-10
**Status**: Draft
**Input**: "Implement a basic model cascade system that automatically selects appropriate models based on task complexity. Phase 1 focuses on conservative routing with a Medium-tier default and relies on the escalate tool for corrections."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic Model Selection Based on Task Complexity (Priority: P1)

A user initiates a task with Hoosh. The system should automatically analyze the task complexity and select an appropriate model tier (Light, Medium, Heavy) without requiring explicit user configuration. This provides a "set and forget" experience where Hoosh intelligently chooses the right compute level.

**Why this priority**: This is the core value proposition of the model cascade system. It simplifies user experience by removing the need for manual model selection while optimizing cost/performance tradeoffs.

**Independent Test**: Can be fully tested by submitting various tasks (simple queries, complex analysis, code refactoring) and verifying that Hoosh selects appropriate models for each complexity level.

**Acceptance Scenarios**:

1. **Given** a user submits a simple task (e.g., "What is 2+2?"), **When** Hoosh initializes the agent, **Then** a Light-tier model is selected for execution
2. **Given** a user submits a moderate complexity task (e.g., "Analyze this codebase and suggest improvements"), **When** Hoosh initializes the agent, **Then** a Medium-tier model is selected
3. **Given** a user submits a complex task (e.g., "Implement a distributed cache system"), **When** Hoosh initializes the agent, **Then** a Heavy-tier model is selected
4. **Given** task complexity cannot be clearly determined, **When** Hoosh initializes the agent, **Then** Medium-tier model is selected as conservative default

---

### User Story 2 - Manual Escalation with Escalate Tool (Priority: P1)

When a model encounters a task that's too complex for its tier (e.g., Medium-tier model unable to solve a problem), the system should provide an escalate tool. The user or system can trigger escalation to upgrade to the next tier (Medium → Heavy).

**Why this priority**: This is critical for reliability. Users need a safety valve when the initial model selection proves insufficient. Phase 1's conservative approach (defaulting to Medium) relies on escalation to handle edge cases.

**Independent Test**: Can be fully tested by selecting a Light-tier model for a complex task, allowing it to attempt the task, and verifying that the escalate tool can promote to Medium-tier.

**Acceptance Scenarios**:

1. **Given** a Light-tier model is executing a task, **When** the model determines the task is too complex, **Then** the model can invoke the `escalate` tool with a reason
2. **Given** the escalate tool is invoked, **When** Medium-tier resources are available, **Then** the task is automatically re-executed with a Medium-tier model maintaining conversation context
3. **Given** a Medium-tier model escalates, **When** invoked, **Then** the task is re-executed with a Heavy-tier model preserving all prior conversation history
4. **Given** escalation occurs, **When** the task completes, **Then** the final result is attributed to the escalated model but includes notes on the escalation path

---

### User Story 3 - Cascade Configuration & Safe Defaults (Priority: P1)

An operator wants to use the model cascade system for automatic model selection and escalation. However, they want cascades to be OFF by default for safety and cost control. Cascades should only be activated explicitly when they configure a `cascades` section in their configuration file with explicit cascade policy definitions.

**Why this priority**: This is critical for production safety. Operators need explicit control over when cascades are enabled. Defaulting to OFF prevents unexpected automatic escalations that could increase costs or change behavior without operator approval.

**Independent Test**: Can be tested by verifying that: (1) with no cascade configuration, Hoosh runs in standard mode without cascade features, and (2) when a cascade configuration is added, cascade features activate.

**Acceptance Scenarios**:

1. **Given** Hoosh is running with no `cascades` section in config, **When** a user submits a task, **Then** cascade routing is NOT used and the system behaves normally
2. **Given** Hoosh is running with no `cascades` section in config, **When** a model would call escalate tool, **Then** the escalate tool is unavailable/errors gracefully
3. **Given** a `cascades` section is added to config with tier definitions, **When** Hoosh restarts, **Then** cascade routing is enabled and escalate tool becomes available
4. **Given** cascade configuration is present, **When** operator removes the `cascades` section and restarts, **Then** cascade features are disabled again

---

### User Story 4 - Conservative Routing with Medium Default (Priority: P1)

To reduce costs and latency in Phase 1, the system should conservatively default to Medium-tier models for uncertain cases. This provides a balanced approach where most tasks complete efficiently without over-provisioning.

**Why this priority**: This is Phase 1's core architectural decision. Conservative routing ensures predictable performance and cost, with escalation handling the exceptional cases.

**Independent Test**: Can be fully tested by submitting ambiguous tasks and verifying that Medium-tier is selected, then confirming escalation works when needed.

**Acceptance Scenarios**:

1. **Given** task complexity is ambiguous, **When** Hoosh routes to a model, **Then** Medium-tier is selected
2. **Given** ten similar tasks of ambiguous complexity, **When** Hoosh routes all of them, **Then** all default to Medium-tier
3. **Given** a Medium-tier model completes a task successfully, **When** the task completes, **Then** no escalation is triggered and cost is minimized
4. **Given** cost optimization is a goal, **When** using Medium-tier defaults, **Then** cost is lower than always using Heavy-tier

---

### User Story 5 - Preserve Conversation Context During Escalation (Priority: P2)

When escalation occurs, the conversation history and prior reasoning should be preserved and available to the escalated model. This enables the higher-tier model to build on prior work rather than starting fresh.

**Why this priority**: This ensures escalation is efficient and effective. Without context preservation, escalation wastes the work done by the lower-tier model and forces the higher-tier model to repeat analysis.

**Independent Test**: Can be fully tested by escalating from one tier to another and verifying that all prior messages and tool calls remain in the conversation.

**Acceptance Scenarios**:

1. **Given** a Light-tier model has completed some analysis and escalates, **When** Medium-tier model takes over, **Then** all prior conversation messages are visible to Medium-tier model
2. **Given** escalation occurs mid-task, **When** the Medium-tier model receives the task, **Then** it can see the Light-tier model's reasoning and tool outputs
3. **Given** a task escalates twice (Light → Medium → Heavy), **When** Heavy-tier model executes, **Then** entire conversation history from all previous tiers is preserved

---

## Clarifications

### Session 2025-12-10

- Q: How should cascade activation be controlled? Should cascades always be active or can they be enabled/disabled? → A: Cascades are OFF by default. They are only activated if a `cascades` section exists in the config file with explicit cascade definitions.

---

### Edge Cases

- What if escalation is requested when already at Heavy-tier (maximum tier)?
- What if a model fails even after escalation to Heavy-tier?
- How does the system handle cost tracking across escalations?
- What if a task completes successfully on a lower tier but the user manually requests escalation for re-analysis?
- How does the system handle network failures during escalation transitions?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST analyze incoming task complexity using multi-signal heuristics (not length alone) before model selection
- **FR-001a**: Complexity analysis MUST consider: structural depth (requirement nesting), action density (task verbs), code signals (presence/cyclomatic complexity), and concept count
- **FR-001b**: Complexity analysis confidence MUST be quantified (0.0-1.0) to drive routing conservatism
- **FR-002**: System MUST categorize tasks into three complexity levels: Light, Medium, Heavy based on complexity score
- **FR-003**: System MUST implement conservative routing that defaults to Medium-tier when complexity is ambiguous (confidence < 0.7)
- **FR-003a**: System MUST keep cascade features OFF by default (no automatic tier selection or escalation enabled)
- **FR-003b**: System MUST ONLY activate cascade features if a `cascades` section exists in the configuration file with explicit cascade definitions
- **FR-004**: System MUST map complexity levels to appropriate model tiers:
  - Light: Fast, cost-effective models (e.g., GPT-4o mini, Claude 3.5 Haiku)
  - Medium: Balanced models (e.g., Claude 3.5 Sonnet, GPT-4o)
  - Heavy: Most capable models (e.g., Claude 3 Opus, GPT-4 Turbo)
- **FR-005**: System MUST provide an `escalate` tool available to all executing models
- **FR-006**: Escalate tool MUST support escalation within the same backend (e.g., Anthropic Light → Medium → Heavy)
- **FR-007**: Escalate tool MUST require a reason parameter explaining why escalation is needed
- **FR-008**: Escalate tool MUST preserve complete conversation history during escalation
- **FR-009**: System MUST prevent unnecessary escalations by validating that task requires higher tier
- **FR-010**: System MUST handle escalation gracefully when maximum tier (Heavy) is already active
- **FR-011**: System MUST track which tier executed each task for monitoring and analysis
- **FR-012**: System MUST support rolling back if escalated model also fails (with appropriate messaging)

### Key Entities

- **TaskComplexity**: Represents complexity level of incoming task with multi-signal analysis
  - Level: One of {Light, Medium, Heavy}
  - Confidence: Float 0.0-1.0 representing confidence in the complexity assessment
  - Reasoning: String explaining why task was assigned this complexity
  - Metrics:
    - StructuralDepth: Nesting level of requirements (1-5 scale; 1-2=Light, 2-3=Medium, 3+=Heavy)
    - ActionDensity: Count of action verbs indicating work (0-1=Light, 2-4=Medium, 5+=Heavy)
    - CodeSignals: Presence and cyclomatic complexity of code blocks
    - ConceptCount: Unique domains/entities mentioned (0-5=Light, 5-10=Medium, 10+=Heavy)

- **ModelTier**: Represents capability tier of a model
  - Tier: One of {Light, Medium, Heavy}
  - Backend: Which AI backend (anthropic, openai, together_ai, ollama)
  - Models: List of specific models in this tier (e.g., ["gpt-4o-mini", "gpt-4o"])
  - MaxCostPerRequest: Estimated max cost in cents

- **CascadeContext**: Maintains state during escalation
  - CurrentTier: Active tier
  - EscalationPath: Vec of tiers used so far
  - OriginalTask: The initial task description
  - ConversationHistory: Complete message history across all tiers
  - TimeStarted: When the cascade began

- **CascadeConfig**: Configuration for cascade activation and policy (optional, must be explicitly defined)
  - Enabled: Boolean derived from config file presence (true if `cascades` section exists, false otherwise)
  - RoutingPolicy: Which complexity analysis method to use (e.g., "multi-signal", "threshold-based")
  - EscalationPolicy: Rules for when escalation is permitted (e.g., "allow_all", "light_to_medium_only")
  - DefaultTier: Default tier if complexity cannot be determined (defaults to "Medium" if cascades enabled)
  - CostLimits: Optional per-tier cost caps to enforce during escalations

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: System correctly identifies and routes tasks with high confidence (>0.8) to appropriate tier 85% of the time using multi-signal metrics
- **SC-002**: Multi-signal routing accuracy is at least 15% higher than length-only routing on equivalent task sets
- **SC-003**: For ambiguous tasks (confidence < 0.7), Medium-tier default correctly completes 85% of tasks without escalation
- **SC-004**: When escalation is needed, tasks complete successfully on escalated tier 98% of the time
- **SC-005**: Conversation history is 100% preserved during escalations with zero message loss
- **SC-006**: Escalation latency is < 2 seconds (excluding LLM response time)
- **SC-007**: Cost savings from conservative Medium-tier default is 30-40% versus always using Heavy-tier for ambiguous tasks
- **SC-008**: System correctly identifies structural depth, action density, and code signals with 80%+ accuracy on human-labeled test set
- **SC-009**: When no cascade configuration exists, system operates in standard mode with cascades disabled (zero escalations triggered)
- **SC-010**: When cascade configuration is added and system restarts, cascades activate correctly 100% of the time

## Assumptions

- Phase 1 focuses on single-backend escalation (e.g., all Anthropic or all OpenAI)
- Cross-backend escalation (e.g., OpenAI to Anthropic) is out of scope for Phase 1
- Task complexity analysis can be performed deterministically or with simple heuristics
- Users have access to all three model tiers within their chosen backend
- Escalations are always "upward" (Light → Medium, Medium → Heavy) never lateral or downward
- Model tier assignments can be statically configured per backend
- Conversation history is always serializable and fits in memory for Phase 1
- Phase 1 does not implement automatic downgrade/optimization after successful escalation
- Cascades are OFF by default and require explicit config file activation
- If no `cascades` section exists in config, the system operates in standard mode (no complexity routing, no escalation)
- Configuration changes (adding/removing `cascades` section) require system restart to take effect

## Appendix: Complexity Metrics Approach

### Why Multi-Signal Routing (Not Length-Only)

**Problem with Length Metric**:
- "Fix typo on line 5" (35 chars) routes as Light ✓ correct
- "Refactor 10,000 LOC microservice" (50 chars) routes as Light ✗ actually Heavy
- "Explain quantum computing in detail" (2000 chars) routes as Heavy ✗ actually Medium

### Recommended Phase 1 Metric Stack

| Metric | Light Signal | Medium Signal | Heavy Signal | Weight |
|--------|--------------|---------------|--------------|--------|
| **Structural Depth** | 1-2 nested levels | 2-3 nested levels | 3+ nested levels | 35% |
| **Action Density** | 0-1 action verbs | 2-4 action verbs | 5+ action verbs | 35% |
| **Code Signals** | No code or simple | Code present, CC < 5 | Complex code, CC 5+ | 30% |
| **Concept Count** | 0-5 unique domains | 5-10 unique domains | 10+ unique domains | (informative) |

**Example Scoring**:
- "What is Docker?" → Depth:1, Verbs:0, Code:No → Light (0.2 confidence)
- "Add OAuth to login flow" → Depth:2, Verbs:1, Code:maybe → Medium (0.75 confidence)
- "Design distributed consensus with Byzantine tolerance" → Depth:3, Verbs:2+, Concepts:6+ → Heavy (0.85 confidence)

### Routing Decision Logic

```
score = (0.35 * depth_normalized) + (0.35 * action_density_normalized) + (0.30 * code_signal)

if score < 0.35 and confidence > 0.80:
    tier = Light
elif score > 0.65 and confidence > 0.75:
    tier = Heavy
else:
    tier = Medium  # Conservative default
```

### Why This Approach

1. **Explainable**: Each signal has clear meaning (not black-box ML)
2. **Debuggable**: Can inspect why a task routed to a tier
3. **Safe**: Conservative Medium default + escalate tool catches errors
4. **Testable**: Each metric can be validated independently
5. **Self-Correcting**: Escalate tool handles routing mismatches
