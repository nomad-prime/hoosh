# ACE-Based Orchestration for Hoosh

## Overview

This document outlines the implementation of **ACE (Agentic Context Engineering)** for Hoosh's multi-agent orchestration system. ACE is a framework that treats contexts as evolving playbooks that accumulate, refine, and organize strategies through generation, reflection, and curation.

## Paper Summary

**Paper**: "Agentic Context Engineering: Evolving Contexts for Self-Improving Language Models"
**Authors**: Zhang et al. (Stanford, SambaNova Systems, UC Berkeley)
**Key Results**:
- +10.6% improvement on agent benchmarks (AppWorld)
- +8.6% improvement on domain-specific benchmarks
- 86.9% reduction in adaptation latency
- Matches top-ranked production agents using smaller open-source models

### Core Concepts

1. **Context as Playbook**: Instead of concise summaries, contexts are comprehensive, evolving playbooks with detailed domain insights
2. **Agentic Architecture**: Three specialized components working together:
   - **Generator**: Produces reasoning trajectories
   - **Reflector**: Distills insights from successes and errors
   - **Curator**: Integrates insights into structured context updates
3. **Incremental Delta Updates**: Localized, itemized updates instead of monolithic rewrites
4. **Grow-and-Refine**: Balance between context expansion and redundancy control

### Key Innovations

1. **Prevents Context Collapse**: Avoids degradation where contexts shrink into less informative summaries
2. **Avoids Brevity Bias**: Maintains detailed, domain-specific knowledge instead of generic instructions
3. **Self-Improving**: Works without labeled supervision by leveraging execution feedback
4. **Efficient**: Lower cost and latency through incremental updates

---

## Architecture for Hoosh

### 1. Multi-Agent System with ACE Orchestration

```toml
# ~/.config/hoosh/config.toml

default_backend = "anthropic"
default_agent = "coder"
orchestration_enabled = false  # Toggle AI-controlled agent switching

[orchestration]
mode = "manual"  # "manual" | "automatic" | "hybrid"
reflector_backend = "anthropic"  # Can use different model for reflection
curator_backend = "anthropic"
max_context_tokens = 100000
enable_grow_and_refine = true
deduplication_threshold = 0.85  # Semantic similarity threshold

[orchestration.multi_epoch]
enabled = true
max_epochs = 5
convergence_threshold = 0.95  # Stop if improvement < 5%

# Agents with specialized roles
[agents.planner]
file = "planner.txt"
description = "Breaks down complex tasks into actionable steps"
tags = ["planning", "architecture", "strategy"]
allowed_tools = ["read_file", "list_files", "grep_tool", "find_tool"]
role = "generator"  # This agent generates plans

[agents.coder]
file = "coder.txt"
description = "Implements features and writes code"
tags = ["coding", "implementation"]
role = "generator"

[agents.reviewer]
file = "reviewer.txt"
description = "Reviews code for bugs, style, and correctness"
tags = ["review", "quality", "testing"]
role = "reflector"  # This agent acts as reflector

[agents.debugger]
file = "debugger.txt"
description = "Analyzes errors and suggests fixes"
tags = ["debugging", "troubleshooting"]
role = "generator"

[agents.architect]
file = "architect.txt"
description = "Makes design decisions for large changes"
tags = ["design", "architecture", "patterns"]
role = "generator"

[agents.orchestrator]
file = "orchestrator.txt"
description = "Decides which agent to use and when to switch"
tags = ["meta", "coordination"]
role = "curator"  # Orchestrator acts as curator
```

### 2. Core Data Structures

```rust
// src/orchestration/mod.rs

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// A single bullet in the evolving context playbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bullet {
    /// Unique identifier for this bullet
    pub id: String,

    /// Which section this bullet belongs to
    pub section: BulletSection,

    /// The actual content (strategy, insight, code snippet, etc.)
    pub content: String,

    /// Metadata tracking usefulness
    pub metadata: BulletMetadata,

    /// Tags for retrieval and filtering
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletMetadata {
    /// Number of times marked as helpful
    pub helpful_count: u32,

    /// Number of times marked as harmful
    pub harmful_count: u32,

    /// Timestamp of creation
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Timestamp of last update
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// Embedding for semantic similarity (optional)
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BulletSection {
    StrategiesAndRules,
    ApiUsage,
    CommonMistakes,
    DomainKnowledge,
    CodeSnippets,
    VerificationChecklist,
    FormulasAndCalculations,
}

/// The evolving context playbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    /// All bullets organized by section
    pub bullets: HashMap<BulletSection, Vec<Bullet>>,

    /// Metadata about the playbook
    pub metadata: PlaybookMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookMetadata {
    pub total_bullets: usize,
    pub total_tokens: usize,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub version: u32,
}

/// Delta update from Curator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaUpdate {
    /// Operations to perform
    pub operations: Vec<Operation>,

    /// Reasoning behind the updates
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    Add {
        section: BulletSection,
        content: String,
        tags: Vec<String>,
    },
    Update {
        bullet_id: String,
        new_content: Option<String>,
        increment_helpful: bool,
        increment_harmful: bool,
    },
    Remove {
        bullet_id: String,
        reason: String,
    },
}

/// Agent handoff information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHandoff {
    /// Summary of work done by previous agent
    pub summary: String,

    /// Relevant context from previous agent
    pub relevant_context: Vec<String>,

    /// Current task state
    pub task_state: TaskState,

    /// Which agent to hand off to
    pub target_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskState {
    Planning { plan: Vec<String> },
    Implementing { current_step: usize, total_steps: usize },
    Testing { tests_passed: usize, tests_failed: usize },
    Debugging { errors: Vec<String> },
    Reviewing { files_reviewed: usize, issues_found: usize },
    Complete { summary: String },
}
```

### 3. ACE Manager

```rust
// src/orchestration/ace_manager.rs

pub struct AceManager {
    /// Current active agent
    current_agent: String,

    /// Available agents
    agents: HashMap<String, Agent>,

    /// The evolving playbook
    playbook: Playbook,

    /// Orchestrator agent (decides switching)
    orchestrator: Option<OrchestratorAgent>,

    /// Configuration
    config: AceConfig,

    /// LLM backends for different roles
    generator_backend: Box<dyn LlmBackend>,
    reflector_backend: Box<dyn LlmBackend>,
    curator_backend: Box<dyn LlmBackend>,
}

impl AceManager {
    /// Initialize ACE system
    pub async fn new(config: AceConfig) -> Result<Self> {
        // Load agents from config
        // Initialize playbook (empty or from saved state)
        // Set up orchestrator if enabled
    }

    /// Execute a task with ACE framework
    pub async fn execute_task(&mut self, task: &str) -> Result<TaskResult> {
        let mut epoch = 0;
        let max_epochs = self.config.max_epochs;

        loop {
            // 1. GENERATION: Current agent generates trajectory
            let trajectory = self.generate_trajectory(task).await?;

            // 2. REFLECTION: Reflector analyzes trajectory
            let reflection = self.reflect_on_trajectory(&trajectory).await?;

            // 3. CURATION: Curator creates delta updates
            let delta = self.curate_updates(&reflection).await?;

            // 4. UPDATE: Merge delta into playbook
            self.update_playbook(delta).await?;

            // 5. REFINE: Deduplicate and prune if needed
            if self.config.enable_grow_and_refine {
                self.refine_playbook().await?;
            }

            // 6. Check convergence or max epochs
            epoch += 1;
            if self.should_stop(epoch, max_epochs, &trajectory) {
                break;
            }

            // 7. Optionally switch agents (orchestration)
            if self.config.orchestration_enabled {
                self.maybe_switch_agent(&trajectory).await?;
            }
        }

        Ok(trajectory.result)
    }

    /// Generator: Produce reasoning trajectory
    async fn generate_trajectory(&self, task: &str) -> Result<Trajectory> {
        let agent = self.get_current_agent();

        // Retrieve relevant bullets from playbook
        let relevant_bullets = self.retrieve_relevant_bullets(task)?;

        // Format playbook context
        let playbook_context = self.format_playbook(&relevant_bullets);

        // Construct prompt with playbook
        let prompt = self.build_generator_prompt(
            task,
            &playbook_context,
            &agent.system_prompt,
        );

        // Generate with LLM
        let response = self.generator_backend.generate(&prompt).await?;

        Ok(Trajectory {
            task: task.to_string(),
            agent: agent.name.clone(),
            reasoning: response.reasoning,
            actions: response.actions,
            result: response.result,
            bullets_used: response.bullet_ids,
        })
    }

    /// Reflector: Analyze trajectory and extract insights
    async fn reflect_on_trajectory(
        &self,
        trajectory: &Trajectory,
    ) -> Result<Reflection> {
        let reflector_prompt = self.build_reflector_prompt(trajectory);

        // Iterative refinement (up to max_refinement_rounds)
        let mut reflection = None;
        for round in 0..self.config.max_refinement_rounds {
            let response = self.reflector_backend
                .generate(&reflector_prompt)
                .await?;

            reflection = Some(response);

            // Could add early stopping if reflection is good enough
        }

        Ok(reflection.unwrap())
    }

    /// Curator: Create delta updates from reflection
    async fn curate_updates(
        &self,
        reflection: &Reflection,
    ) -> Result<DeltaUpdate> {
        let curator_prompt = self.build_curator_prompt(
            reflection,
            &self.playbook,
        );

        let delta = self.curator_backend
            .generate(&curator_prompt)
            .await?;

        Ok(delta)
    }

    /// Update playbook with delta
    async fn update_playbook(&mut self, delta: DeltaUpdate) -> Result<()> {
        for operation in delta.operations {
            match operation {
                Operation::Add { section, content, tags } => {
                    let bullet = Bullet {
                        id: self.generate_bullet_id(),
                        section,
                        content,
                        metadata: BulletMetadata::new(),
                        tags,
                    };

                    self.playbook.add_bullet(bullet);
                }
                Operation::Update { bullet_id, new_content, increment_helpful, increment_harmful } => {
                    self.playbook.update_bullet(
                        &bullet_id,
                        new_content,
                        increment_helpful,
                        increment_harmful,
                    )?;
                }
                Operation::Remove { bullet_id, reason } => {
                    self.playbook.remove_bullet(&bullet_id)?;
                }
            }
        }

        self.playbook.metadata.version += 1;
        self.playbook.metadata.last_updated = chrono::Utc::now();

        Ok(())
    }

    /// Grow-and-refine: Deduplicate and prune
    async fn refine_playbook(&mut self) -> Result<()> {
        // 1. Semantic deduplication using embeddings
        let duplicates = self.find_duplicate_bullets().await?;
        for dup in duplicates {
            self.merge_or_remove_bullet(dup)?;
        }

        // 2. Prune low-value bullets (high harmful_count, low helpful_count)
        let to_prune = self.find_bullets_to_prune()?;
        for bullet_id in to_prune {
            self.playbook.remove_bullet(&bullet_id)?;
        }

        // 3. If context exceeds token limit, aggressive pruning
        if self.playbook.metadata.total_tokens > self.config.max_context_tokens {
            self.aggressive_prune()?;
        }

        Ok(())
    }

    /// Orchestrator: Decide if agent should switch
    async fn maybe_switch_agent(&mut self, trajectory: &Trajectory) -> Result<()> {
        if let Some(orchestrator) = &self.orchestrator {
            let decision = orchestrator.decide_next_agent(trajectory).await?;

            if decision.should_switch {
                let handoff = self.create_handoff(
                    &self.current_agent,
                    &decision.target_agent,
                    trajectory,
                );

                self.switch_agent(&decision.target_agent, handoff)?;
            }
        }

        Ok(())
    }

    /// Switch to a different agent
    pub fn switch_agent(
        &mut self,
        target_agent: &str,
        handoff: AgentHandoff,
    ) -> Result<()> {
        if !self.agents.contains_key(target_agent) {
            return Err(anyhow!("Agent '{}' not found", target_agent));
        }

        println!("🔄 Switching from [{}] to [{}]", self.current_agent, target_agent);
        println!("Context: {}", handoff.summary);

        self.current_agent = target_agent.to_string();

        // Store handoff context for new agent
        // This becomes part of the context for the next generation

        Ok(())
    }

    /// Retrieve relevant bullets for a task
    fn retrieve_relevant_bullets(&self, task: &str) -> Result<Vec<Bullet>> {
        // Could use:
        // 1. Semantic search with embeddings
        // 2. Keyword matching
        // 3. Tag filtering
        // 4. Recency and usefulness scoring

        // For now, simple approach: return all bullets sorted by usefulness
        let mut all_bullets: Vec<Bullet> = self.playbook
            .bullets
            .values()
            .flatten()
            .cloned()
            .collect();

        all_bullets.sort_by_key(|b| {
            std::cmp::Reverse(b.metadata.helpful_count as i32 - b.metadata.harmful_count as i32)
        });

        Ok(all_bullets)
    }
}
```

### 4. Commands for Agent Toggling

```rust
// src/commands/agent_commands.rs

/// List all available agents
pub fn list_agents(ace_manager: &AceManager) {
    println!("Available Agents:");
    for (name, agent) in &ace_manager.agents {
        let current = if name == &ace_manager.current_agent { " (current)" } else { "" };
        println!("  - {} {}{}", name, agent.description, current);
        println!("    Tags: {}", agent.tags.join(", "));
    }
}

/// Switch to a specific agent
pub fn switch_agent(ace_manager: &mut AceManager, agent_name: &str) -> Result<()> {
    let handoff = AgentHandoff {
        summary: format!("Manual switch to {}", agent_name),
        relevant_context: vec![],
        task_state: TaskState::Planning { plan: vec![] },
        target_agent: agent_name.to_string(),
    };

    ace_manager.switch_agent(agent_name, handoff)
}

/// Toggle orchestration mode
pub fn toggle_orchestration(ace_manager: &mut AceManager) {
    ace_manager.config.orchestration_enabled = !ace_manager.config.orchestration_enabled;
    let status = if ace_manager.config.orchestration_enabled { "enabled" } else { "disabled" };
    println!("Orchestration mode: {}", status);
}

/// Show current playbook stats
pub fn show_playbook_stats(ace_manager: &AceManager) {
    let pb = &ace_manager.playbook;
    println!("Playbook Statistics:");
    println!("  Version: {}", pb.metadata.version);
    println!("  Total bullets: {}", pb.metadata.total_bullets);
    println!("  Total tokens: {}", pb.metadata.total_tokens);
    println!("  Last updated: {}", pb.metadata.last_updated);
    println!("\nBullets by section:");
    for (section, bullets) in &pb.bullets {
        println!("  {:?}: {}", section, bullets.len());
    }
}
```

### 5. Command Integration

```rust
// In src/tui/command_handler.rs

match command.as_str() {
    "/agents" | "/agent list" => {
        list_agents(&self.ace_manager);
    }
    cmd if cmd.starts_with("/agent ") => {
        let agent_name = cmd.strip_prefix("/agent ").unwrap();
        switch_agent(&mut self.ace_manager, agent_name)?;
    }
    "/toggle" => {
        // Cycle through agents
        self.ace_manager.cycle_to_next_agent()?;
    }
    "/orchestrate" => {
        toggle_orchestration(&mut self.ace_manager);
    }
    "/playbook" => {
        show_playbook_stats(&self.ace_manager);
    }
    "/playbook export" => {
        self.ace_manager.export_playbook("./playbook.json")?;
    }
    "/playbook import" => {
        self.ace_manager.import_playbook("./playbook.json")?;
    }
    _ => {}
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)
- [ ] Implement core data structures (Bullet, Playbook, DeltaUpdate)
- [ ] Basic AceManager with generation-only (no reflection/curation yet)
- [ ] Agent switching commands (`/agent`, `/agents`, `/toggle`)
- [ ] Playbook serialization and persistence

### Phase 2: Reflection & Curation (Week 3-4)
- [ ] Implement Reflector component with iterative refinement
- [ ] Implement Curator component with delta updates
- [ ] Incremental playbook updates
- [ ] Grow-and-refine mechanism

### Phase 3: Orchestration (Week 5-6)
- [ ] Implement Orchestrator agent
- [ ] Automatic agent switching based on task state
- [ ] Agent handoff mechanism with context passing
- [ ] Hybrid mode (manual + automatic switching)

### Phase 4: Optimization (Week 7-8)
- [ ] Semantic search for bullet retrieval
- [ ] Embedding-based deduplication
- [ ] Multi-epoch adaptation with convergence detection
- [ ] Performance profiling and optimization

---

## Benefits for Hoosh

1. **Self-Improving**: Hoosh learns from its own execution and improves over time
2. **Specialized Agents**: Different agents for different tasks (planning, coding, reviewing, debugging)
3. **Flexible Orchestration**: Manual, automatic, or hybrid agent switching
4. **Context Preservation**: Detailed knowledge accumulates instead of being compressed away
5. **Efficient**: Incremental updates reduce latency and cost
6. **Interpretable**: Playbook is human-readable and editable

---

## References

- Zhang et al. (2025). "Agentic Context Engineering: Evolving Contexts for Self-Improving Language Models"
- AppWorld Benchmark: https://appworld.dev/leaderboard
- Dynamic Cheatsheet: https://github.com/suzgunmirac/dynamic-cheatsheet
