---
name: lessons-learner
description: Record and retrieve past mistakes and API usage lessons
---

# Lessons Learner Skill

This skill acts as your long-term memory for mistakes and lessons. It ensures that once a mistake is made and solved, it is never repeated.

## Storage Location

**Knowledge Base**: `~/.happy_coding/knowledge/`
**Structure**: Category-based Markdown files.

## Classification Philosophy

**Think Multi-dimensionally.** A single problem often touches multiple domains.
- **Goal**: Minimize redundancy, Maximize discoverability.
- **Strategy**: Record full detail in the **Primary Category**, add lightweight links in **Related Categories**.

### Recommended Categories
- `language-[lang].md`: Syntax, core library quirks (e.g., `language-rust.md`).
- `framework-[name].md`: Framework specific (e.g., `framework-react.md`, `framework-tokio.md`).
- `tooling.md`: CI/CD, Git, Docker, Build tools.
- `architecture.md`: Design patterns, layering, modularity.
- `security.md`: Auth, encryption, common vulnerabilities.
- `performance.md`: Optimization techniques, benchmarking.
- `process.md`: Workflow, git conventions, review guidelines.

## Core Actions

### 1. Check Lessons (Before Coding)

**Trigger**: Before using a new library, calling a complex API, or starting a generic task.
**Instruction**: 
1. **Analyze Context**: What technologies and domains are involved?
2. **List Categories**: `ls ~/.happy_coding/knowledge/`
3. **Select & Read**: Read potentially relevant files.

```bash
# Example: Starting a Rust Async Web Project
ls ~/.happy_coding/knowledge/
cat ~/.happy_coding/knowledge/language-rust.md 2>/dev/null
cat ~/.happy_coding/knowledge/framework-tokio.md 2>/dev/null
# Check performance too if relevant
cat ~/.happy_coding/knowledge/performance.md 2>/dev/null
```

### 2. Record Lesson (After Fixing)

**Trigger**: After spending >10 mins debugging an issue, or realizing a misunderstanding of an API.
**Instruction**: 
1. **Identify Primary Category**: Where does this lesson belong most? (e.g. `framework-tokio.md` for async runtime issues).
2. **Identify Related Categories**: Does it affect others? (e.g. `performance.md` if it caused slow-down).
3. **Record**:
    - **Primary**: Append full lesson.
    - **Secondary**: Append a **Link Pointer**.

**Format (Primary)**:
```markdown
### [Tag/Keyword] Short Title
- **Context**: ...
- **Bad Approach**: ...
- **Root Cause**: ...
- **Correct Approach**: ...
- **Date**: YYYY-MM-DD
```

**Format (Secondary Link)**:
```markdown
- [Tag/Keyword] See `framework-tokio.md` for details on "Blocking code in async context".
```

## Example Workflow

**Scenario**: You fixed a deadlock in Rust async code caused by blocking thread. It caused severe performance degradation.

**Thinking Process**:
- **Primary**: This is a `framework-tokio` usage error.
- **Related**: It impacts `performance`.

**Action**:
1.  **Write Full Lesson** to `framework-tokio.md`.
2.  **Add Link** to `performance.md`:
    ```bash
    echo "- [Async/Deadlock] blocking code caused performance drop. See framework-tokio.md" >> ~/.happy_coding/knowledge/performance.md
    ```

## Initialization

If the knowledge directory does not exist, create it:

```bash
mkdir -p ~/.happy_coding/knowledge
touch ~/.happy_coding/knowledge/general.md
```




