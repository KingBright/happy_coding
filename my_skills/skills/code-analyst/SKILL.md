---
name: code-analyst
description: Use this skill to analyze an existing codebase. It generates a comprehensive report covering project background, technical architecture, key challenges, and technology choices. It can also perform competitive analysis if requested.
---

# Code Analyst Skill

This skill turns you into a senior technical auditor. Use it when the user asks to "understand this project", "analyze the codebase", or "explain how this works".

## Core Responsibilities

1.  **Context Extraction**: Quickly identify the project's purpose and domain.
2.  **Architecture Mapping**: Visualize the high-level design and data flow.
3.  **Critical Analysis**: Identify technical challenges and trade-offs.
4.  **Comparative Study**: Contrast with known industry alternatives.

## Workflow

### Step 1: Reconnaissance (Broad Scan)

Start by identifying the project skeleton.

- **Check Metadata**: `package.json`, `Cargo.toml`, `go.mod`, `pom.xml`.
- **Read Documentation**: `README.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`.
- **List Files**: `ls -R` or `find` to see distribution of code.

### Step 2: Architecture Discovery (Deep Dive)

Trace the entry points and data flow.

- **Entry Points**: `main.rs`, `index.js`, `App.tsx`, `Controller` classes.
- **Core Logic**: Look for `domain`, `service`, `core` directories.
- **Infrastructure**: Look for `db`, `api`, `adapters`.

### Step 3: Analysis Reporting

Generate a report using the template below.

## Output Template: Project Analysis Report

```markdown
# Project Analysis: [Project Name]

## 1. Project Background
- **One-Liner**: What does it do?
- **Target Audience**: Who is it for?
- **Business Value**: What problem does it solve?

## 2. Technical Architecture
- **Tech Stack**: [Language, Frameworks, DB, Build Tools]
- **Design Pattern**: [e.g., MVC, Hexagonal, Microservices, Monolith]
- **Structure**:
    - `Layer A`: Responsibilities...
    - `Layer B`: Responsibilities...

## 3. Key Technical Challenges
- **Challenge 1**: [e.g., Real-time synchronization]
    - *Solution found*: [e.g., WebSocket + CRDTs]
- **Challenge 2**: [e.g., Cross-platform support]
    - *Solution found*: [e.g., Rust core + Adapters]

## 4. Competitive Analysis (If applicable)
| Feature | This Project | Competitor A (e.g., X) | Competitor B (e.g., Y) |
|---------|--------------|------------------------|------------------------|
| Tech Stack | Rust | Node.js | Go |
| Performance | High | Medium | High |
| Ecosystem | Growing | Mature | Mature |

## 5. Summary & Recommendations
- **Strengths**: ...
- **Weaknesses**: ...
- **Next Steps**: [Suggestions for the user]
```

## Tips for Agents

- **Don't Guess**: If you can't find specific implementation details, say "Not found".
- **Look for "Hacks"**: Search for specific comments like `TODO`, `FIXME`, or complex workaround code to find "Technical Challenges".
- **External Knowledge**: Use your training data to fill in the "Comparative Analysis" section (e.g., if analyzing a React framework, compare with Vue/Angular).

### Step 4: Archiving

Save the generated report to a persistent file for future reference.

1.  **Create Directory**: Ensure `.code_analysis` directory exists in project root.
2.  **Save File**: Write the report to `.code_analysis/ANALYSIS_REPORT.md`.
    - If the file exists, append a new entry with the current date, OR create a new file with a timestamp (e.g., `ANALYSIS_2024_02_05.md`).







