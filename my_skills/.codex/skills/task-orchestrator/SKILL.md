---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
---

---
name: task-orchestrator
description: Use this skill when managing complex, multi-step implementation tasks. It acts as a project manager to break down requirements into specific tasks, track progress, and ensure orderly execution. Activate when the user says "start a new task", "implement feature X", or when a request is complex enough to require a plan.
---

# Task Orchestrator Skill

This skill acts as your internal project manager. It helps you maintain context, track progress, and execute complex tasks systematically.

## Core Principles

1.  **Plan First**: Never start coding without a clear plan.
2.  **Break It Down**: Divide large goals into small, verifiable steps.
3.  **Track Progress**: Maintain a `task.md` to track what's done and what's next.
4.  **Verify Often**: Each step must be verified before moving to the next.

## Workflow

### 1. Initialization Phase

When a new complex task starts:

1.  **Analyze**: Read all relevant files to understand the current state.
2.  **Plan**: Create a `task.md` file in the root (or update usage of `task_boundary` tool if in Agentic Mode).
3.  **Confirm**: Ask the user to review the plan before execution.

### 2. Execution Phase

For each step in the plan:

1.  **Context**: "I am starting step X: [Description]"
2.  **Action**: Perform the necessary code changes.
3.  **Verification**: Run tests or manual checks to confirm the change works.
4.  **Update**: Mark the step as complete in `task.md`.

### 3. Completion Phase

1.  **Final Verification**: Ensure all requirements are met.
2.  **Cleanup**: Remove temporary files or build artifacts.
3.  **Report**: Summarize what was done and any follow-up actions needed.

## Template: implementation_plan.md

```markdown
# Implementation Plan - [Feature Name]

## Goal
[Brief description of what we are building]

## Proposed Changes
### Component A
- [ ] Change 1
- [ ] Change 2

### Component B
- [ ] Change 3

## Verification Plan
- [ ] Automated Test A
- [ ] Manual Check B
```

## Template: task.md

```markdown
# Task: [Task Name]

- [ ] **Phase 1: Setup**
    - [ ] Step 1.1
    - [ ] Step 1.2
- [ ] **Phase 2: Implementation**
    - [ ] Step 2.1
    - [ ] Step 2.2
- [ ] **Phase 3: Verification**
    - [ ] Step 3.1
```







