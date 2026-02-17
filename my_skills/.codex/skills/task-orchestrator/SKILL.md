---
name: task-orchestrator
description: Manage complex, multi-step implementation tasks
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

---

## Extended Pattern: Autonomous Coding Workflow

For long-running autonomous development tasks, use this structured workflow:

### Session Startup Protocol

Always begin by getting your bearings:

```bash
# 1. See working directory
pwd

# 2. List files to understand structure
ls -la

# 3. Read project specification
cat app_spec.txt

# 4. Read feature list
cat feature_list.json | head -50

# 5. Read progress notes
cat claude-progress.txt

# 6. Check git history
git log --oneline -20

# 7. Count remaining tests
cat feature_list.json | grep '"passes": false' | wc -l
```

### Feature Implementation Cycle

```
├── 1. VERIFICATION TEST (Mandatory Before New Work)
│   └── Run 1-2 core passing tests to ensure no regressions
│   └── Fix any issues before proceeding
│
├── 2. SELECT ONE FEATURE
│   └── Find highest-priority feature with "passes": false
│   └── Focus on completing it perfectly in this session
│
├── 3. IMPLEMENT
│   └── Write code (frontend/backend as needed)
│   └── Test manually using browser automation
│   └── Fix issues, verify end-to-end
│
├── 4. VERIFY WITH BROWSER AUTOMATION (Critical)
│   └── Navigate to app in real browser
│   └── Interact like human (click, type, scroll)
│   └── Take screenshots at each step
│   └── Check for console errors
│   └── Verify BOTH functionality AND visual appearance
│
└── 5. UPDATE PROGRESS
    └── Change "passes": false to "passes": true
    └── Make git commit with descriptive message
    └── Update claude-progress.txt
```

### Quality Standards

Before marking any feature complete:

- [ ] Zero console errors
- [ ] Polished UI matching design spec
- [ ] All features work end-to-end through UI
- [ ] Fast, responsive, professional
- [ ] Screenshots captured in verification/ directory

### Testing Requirements

**MANDATORY:** All testing must use browser automation tools.

Available tools:
- `puppeteer_navigate` - Start browser and go to URL
- `puppeteer_screenshot` - Capture screenshot
- `puppeteer_click` - Click elements
- `puppeteer_fill` - Fill form inputs

**DO:**
- Test through UI with clicks and keyboard input
- Take screenshots to verify visual appearance
- Check for console errors
- Verify complete user workflows

**DON'T:**
- Only test with curl commands
- Use JavaScript evaluation to bypass UI
- Skip visual verification
- Mark tests passing without thorough verification

### Progress Tracking File: claude-progress.txt

Update this file every session:

```markdown
## Session [Date]

### Accomplished
- Implemented [feature name]
- Fixed [issue description]

### Tests Completed
- Test #X: [description] - PASSING

### Issues Discovered
- [Issue and resolution]

### Next Steps
- Implement [next feature]

### Status
X/200 tests passing (X%)
```

### feature_list.json Format

```json
{
  "tests": [
    {
      "id": 1,
      "feature": "User Authentication",
      "description": "Users can log in with email/password",
      "steps": [
        "Navigate to /login",
        "Enter valid credentials",
        "Click login button",
        "Verify redirect to dashboard"
      ],
      "passes": false
    }
  ]
}
```

**CRITICAL:** You can ONLY modify the `"passes"` field. Never:
- Remove tests
- Edit test descriptions
- Modify test steps
- Reorder tests

### Session End Checklist

Before terminating:

1. [ ] Commit all working code
2. [ ] Update claude-progress.txt
3. [ ] Update feature_list.json if tests verified
4. [ ] No uncommitted changes
5. [ ] App left in working state
6. [ ] All broken tests fixed








