# Feature Specification: Daemon Mode

**Feature Branch**: `004-daemon-mode`
**Created**: 2026-03-14
**Status**: Draft
**Input**: https://github.com/nomad-prime/hoosh/issues/51

## Clarifications

### Session 2026-03-14

- Q: How should task workspaces (Sandboxes) be isolated from each other? → A: Separate temp directory per task on host filesystem (no containers). Container-level isolation is a deployment concern — users who need it can run hoosh inside containers themselves.
- Q: Is there a maximum number of tasks that may run concurrently, or is the queue unbounded? → A: Unbounded — all submitted tasks run immediately in parallel.
- Q: What happens when two tasks target the same repository simultaneously? → A: Allowed — each task clones into its own sandbox and pushes to a unique branch (e.g. `hoosh/<task-id>`). Git manages conflicts at its own level, just as when multiple developers work in parallel.
- Q: What happens when the daemon is shut down while a task is running? → A: Graceful by default — stop accepting new tasks, wait for running tasks to finish, then exit. With `--force` flag: cancel all running tasks immediately (commit partial changes with `[incomplete]` marker) and exit.
- Q: Can clients retrieve execution logs/output for a task via the API? → A: Yes — a log retrieval endpoint per task ID.
- Additional constraint: Each task MUST have a configurable maximum token budget. When the budget is exhausted the task is halted, partial changes are committed with an `[incomplete]` marker, and the task is marked failed — preventing runaway agent costs.
- Q: How does the daemon authenticate to remote repositories for cloning, pushing, and opening PRs? → A: SSH key (from `~/.ssh` or configured path) for git operations; PAT stored in daemon config for PR API calls. Credential setup is an admin/operator concern and out of scope for this feature — the daemon assumes both are already configured.
- Q: Which git hosting platforms must be supported for PR creation in v1? → A: GitHub only. The PR provider is implemented behind a trait so other platforms can be added later.
- Q: How is the permission system structured? → A: Hoosh already has a comprehensive permission system. Daemon mode adds a two-level file resolution: `~/.hoosh/permissions.json` (global, admin-managed) loads first as the baseline; `<repo>/.hoosh/permissions.json` (project-managed) merges on top. Allow rules are additive; deny rules always win regardless of level. A project allow that conflicts with a global deny is silently dropped. If no project file exists, global applies as-is. Permission setup is an operator concern and out of scope.
- Q: What happens to in-flight tasks if the daemon crashes unexpectedly? → A: On restart, any task persisted as `running` is marked `failed` with an `[incomplete]` marker. No recovery is attempted.
- Q: What happens if sandbox disk space is exhausted mid-task? → A: Treat as fatal — halt the agent, commit any partial changes with an `[incomplete]` marker, mark the task failed, and clean up the sandbox. Consistent with token exhaustion and permission denial handling.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Submit a Coding Task and Receive a PR (Priority: P1)

A developer wants to delegate a coding task to the daemon without monitoring it. They submit a task specifying a repository and instructions. The daemon autonomously clones the repo, runs the agent, commits any changes, and opens a pull request. The developer reviews the PR at their convenience.

**Why this priority**: This is the core value proposition of daemon mode — fully automated, unattended code changes delivered as reviewable PRs.

**Independent Test**: Can be tested by starting the daemon, submitting a single task with a valid repo URL and instructions, and verifying that a PR is created (or the task is marked completed with no changes if nothing was modified).

**Acceptance Scenarios**:

1. **Given** the daemon is running and a repo URL and instructions are provided, **When** a task is submitted, **Then** the daemon accepts the task, returns a task ID, and asynchronously runs the agent
2. **Given** a submitted task where the agent makes code changes, **When** the agent finishes, **Then** the changes are committed to a dedicated branch and a pull request is opened against the target branch
3. **Given** a submitted task where the agent makes no changes, **When** the agent finishes, **Then** the task is marked completed with no PR created (not an error)
4. **Given** a submitted task where the agent encounters a permission denial it cannot work around, **When** execution halts, **Then** any partial changes are committed with an `[incomplete]` marker in the PR body and the task status is set to failed

---

### User Story 2 - Monitor and Cancel Tasks (Priority: P2)

A developer or CI system needs visibility into running tasks — what is queued, what is running, and what completed or failed. They also need to be able to cancel a task that is no longer needed.

**Why this priority**: Without monitoring, the daemon is a black box. Status visibility and cancellation are essential for operational confidence.

**Independent Test**: Can be tested by submitting multiple tasks, polling the task list and individual task status endpoints, and verifying that cancelling a running task halts it.

**Acceptance Scenarios**:

1. **Given** one or more tasks have been submitted, **When** the task list is queried, **Then** all active and recent tasks are returned with their current status
2. **Given** a task is running, **When** the task status is polled, **Then** the response reflects the current state (queued, running, completed, failed, or cancelled)
3. **Given** a task is running, **When** a cancel request is issued for that task, **Then** the task is stopped and its status transitions to cancelled
4. **Given** the daemon is operational, **When** the health endpoint is queried, **Then** a liveness confirmation is returned
5. **Given** a task has been run (in any state), **When** the log endpoint is queried with that task's ID, **Then** the execution log for that task is returned

---

### User Story 3 - Control What the Daemon Is Allowed to Do (Priority: P3)

An administrator defines a global baseline of what operations the daemon may perform. Individual repository maintainers can adjust these permissions for their own repo within the bounds the admin has set. This prevents a rogue task from deleting files, force-pushing, or running arbitrary shell commands.

**Why this priority**: Without permission controls, giving an autonomous agent access to a codebase is too risky for most teams. This is a gate on organisational adoption.

**Independent Test**: Can be tested by configuring a global deny on a specific operation, submitting a task that requires that operation, and verifying that the task fails cleanly with an `[incomplete]` marker on any partial work.

**Acceptance Scenarios**:

1. **Given** a global permission file exists, **When** a task runs, **Then** only operations permitted by the global rules are allowed
2. **Given** both global and repo-level permission files exist, **When** a task runs, **Then** repo-level allow rules extend the global baseline and repo-level deny rules further restrict it
3. **Given** a repo-level file attempts to grant a permission that the global file denies, **When** a task runs, **Then** the conflicting repo allow is silently dropped and the global deny wins
4. **Given** no repo-level file exists, **When** a task runs, **Then** only the global permissions apply

---

### User Story 4 - Start, Stop, and Inspect the Daemon via CLI (Priority: P4)

A developer or system operator manages the daemon lifecycle from the command line — starting it, stopping it, and checking whether it is running — without needing to interact with the HTTP interface directly.

**Why this priority**: Operational simplicity for developers who prefer the terminal over constructing HTTP requests.

**Independent Test**: Can be tested on a single machine by running `hoosh daemon start`, verifying the process is active via `hoosh daemon status`, then stopping it with `hoosh daemon stop`.

**Acceptance Scenarios**:

1. **Given** the daemon is not running, **When** `hoosh daemon start` is run, **Then** the daemon starts and accepts incoming task requests
2. **Given** the daemon is running, **When** `hoosh daemon stop` is run, **Then** the daemon stops accepting new tasks, waits for running tasks to complete, and exits
3. **Given** the daemon is running with active tasks, **When** `hoosh daemon stop --force` is run, **Then** all running tasks are cancelled immediately (partial changes committed with `[incomplete]`), and the daemon exits
4. **Given** the daemon may or may not be running, **When** `hoosh daemon status` is run, **Then** the current state is reported
5. **Given** the daemon is running, **When** a task is submitted via `hoosh daemon submit`, **Then** a task ID is returned and the task enters the queue

---

### Edge Cases

- What happens when a submitted repo URL is unreachable or the clone fails?
- What happens if the task's working branch already exists on the remote?
- Graceful shutdown (`hoosh daemon stop`): the daemon stops accepting new tasks and waits for all running tasks to finish before exiting. Force shutdown (`hoosh daemon stop --force`): all running tasks are cancelled immediately, partial changes are committed with an `[incomplete]` marker, then the daemon exits.
- If sandbox disk space is exhausted mid-task: the agent is halted, partial changes are committed with an `[incomplete]` marker, the task is marked failed, and the sandbox is cleaned up — identical to token exhaustion and permission denial handling.
- When a task exhausts its token budget, partial changes are committed with an `[incomplete]` marker and the task is marked failed — identical handling to a permission denial halt.
- If the daemon process crashes (OOM, SIGKILL, etc.), any task that was `running` at crash time is marked `failed` with an `[incomplete]` marker on the next restart. No mid-task recovery is attempted.
- Two tasks targeting the same repository are allowed; each clones independently into its own sandbox and pushes to a unique branch (`hoosh/<task-id>`). Git resolves any content conflicts at PR review time, as with any parallel developer workflow.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST accept task submissions containing a repository URL, a base branch, and freeform instructions
- **FR-002**: The system MUST assign a unique task ID to every submitted task and return it immediately to the caller
- **FR-003**: Each task MUST run in an isolated workspace that is independent from other concurrently running tasks
- **FR-004**: The system MUST clone the target repository into the task's isolated workspace before the agent runs
- **FR-005**: The agent MUST run with the merged permission set resulting from global permissions overlaid with any repo-level overrides
- **FR-006**: On successful agent completion with changes, the system MUST commit the changes, push to a dedicated branch named `hoosh/<task-id>`, and open a pull request
- **FR-007**: On successful agent completion with no changes, the system MUST mark the task completed without creating a PR
- **FR-008**: If agent execution is halted by a permission denial, the system MUST commit any partial changes with an `[incomplete]` marker and mark the task failed
- **FR-009**: The system MUST expose an endpoint to list all active and recent tasks
- **FR-010**: The system MUST expose an endpoint to retrieve the status of a single task by ID
- **FR-011**: The system MUST expose an endpoint to cancel a running task
- **FR-012**: The system MUST expose a health check endpoint for liveness verification
- **FR-018**: The system MUST expose an endpoint to retrieve the execution log of a task by ID
- **FR-019**: Each task MUST run within a configurable maximum token budget; the budget is set globally in config and MAY be overridden per task submission
- **FR-020**: When a task exhausts its token budget, the system MUST halt the agent, commit any partial changes with an `[incomplete]` marker, and mark the task failed
- **FR-013**: The daemon MUST load `~/.hoosh/permissions.json` as the global permission baseline for every task
- **FR-014**: If `<repo>/.hoosh/permissions.json` exists, its rules MUST be merged on top of the global baseline: allow rules are additive, deny rules always win regardless of level, and any project allow that conflicts with a global deny is silently dropped
- **FR-015**: The daemon MUST be startable and stoppable via CLI commands; `hoosh daemon stop` performs a graceful drain and `hoosh daemon stop --force` performs an immediate shutdown with cancellation of all running tasks
- **FR-016**: The daemon MUST support submitting tasks via CLI as a convenience alternative to direct HTTP interaction
- **FR-017**: The daemon MUST bind to localhost by default
- **FR-023**: On startup, the daemon MUST check for persisted tasks in `running` state (indicating a prior crash) and transition them to `failed` with an `[incomplete]` marker before accepting new work
- **FR-021**: The daemon MUST use the SSH key available in the environment (e.g. `~/.ssh`) for all git operations (clone, push). Credential setup is an operator concern and out of scope.
- **FR-022**: The daemon MUST use a PAT configured in the daemon config for all PR API calls against the GitHub REST API. GitHub is the only supported hosting platform in v1. The PR provider MUST be implemented behind a trait/interface so additional platforms (GitLab, Bitbucket, etc.) can be added without touching core task execution logic.

### Key Entities

- **Task**: A unit of autonomous work. Has a unique ID, status (queued, running, completed, failed, cancelled), target repository URL, base branch, instructions, optional PR title and labels, timestamps, and an optional token budget override (falls back to the global default if not set).
- **Permission Rule**: An allow or deny entry in Hoosh's existing permission format. Resolved from two files in order: `~/.hoosh/permissions.json` (global baseline, admin-managed) then `<repo>/.hoosh/permissions.json` (project overrides, repo-managed). Deny always wins; project allows cannot exceed what the global permits.
- **Sandbox**: A temporary directory created per task on the host filesystem. Contains the cloned repository, scratch space, the original task payload, and an execution log. The execution log is retained until the sandbox is cleaned up and is accessible via the log retrieval API endpoint. OS-level process isolation is not applied; container-level isolation is a deployment concern left to the operator.
- **Pull Request**: The output artifact of a task that completed with code changes. Opened on the remote repository against the specified base branch.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can submit a task and receive a pull request without performing any manual steps between submission and PR review
- **SC-002**: All submitted tasks run immediately in parallel — there is no queue depth limit or concurrency cap
- **SC-003**: A permission denial halts only the affected task; all other running tasks continue unaffected
- **SC-004**: Every task reaches a terminal state (completed, failed, or cancelled) — no tasks remain stuck in a non-terminal state indefinitely
- **SC-005**: A cancel request causes the target task to stop within a reasonable time and report the cancelled status on next poll
- **SC-006**: The daemon starts and responds to health checks within a few seconds of launch
- **SC-007**: A task that fails mid-run due to a permission denial or token budget exhaustion still delivers its partial changes as a reviewable PR with a clear incomplete marker
- **SC-008**: A task cannot consume more tokens than its configured budget; the daemon enforces this limit regardless of agent behaviour

## Assumptions

- Authentication on the HTTP endpoint is out of scope for v1; network-level controls (localhost-only binding, reverse proxy with auth) are the recommended mitigation
- Multi-repo tasks, task dependencies, agent-to-agent delegation, result webhooks, and a web UI are explicitly excluded from v1 scope
- Sandbox cleanup after task completion is the default behaviour; retention for debugging purposes is configurable
- The daemon never pushes directly to the default branch; branch protection rules on the remote are the safety net for this
- Task status polling is the only notification mechanism; no push notifications or webhooks in v1
