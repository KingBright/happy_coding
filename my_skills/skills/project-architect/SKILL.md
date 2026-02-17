---
name: project-architect
description: Use this skill when starting a NEW project or module. It focuses on converting business efficiency into a testable, modular technical foundation. It enforces TDD readiness by defining clear interfaces between layers and ensures project continuity through rigorous plan management.
---

# Project Architect Skill

This skill specializes in the "Day 0" phase of development. It bridges the gap between a Product Requirements Document (PRD) and the first line of code, ensuring the project is "Born for TDD".

## Core Responsibilities

1.  **Requirement Analysis**: Extract technical constraints and domain models.
2.  **Interface-First Design**: Define boundaries between modules/layers to support Test Driven Development (TDD).
3.  **Documentation Strategy**: Setup living documentation (`README`, `ARCHITECTURE`, `task.md`) to allow context resumption at any time.
4.  **Phasing**: Break down implementation into verifiable steps.

## Workflow

### Step 1: Requirement Intake

Ask the user for:
- **Business Goal**: What problem are we solving?
- **Scope**: What is included/excluded?
- **Tech Stack**: User preferences.
- **Constraints**: Performance, Security, Timeline.

### Step 1.5: Check Shared Knowledge (Pitfalls)

Before designing, check the shared database for relevant lessons.

```text
Action: List categories `ls ~/.happy_coding/knowledge/` and read relevant files (e.g. `cat ~/.happy_coding/knowledge/rust.md`)
Context: "Checking for known pitfalls related to [Tech Stack]..."
```

### Step 2: Structural & Interface Design (TDD Focus)

Propose a project structure and **explicitly define interfaces** between components.

**Key Principle**: Each module must be testable in isolation.
- Use **Traits/Interfaces** to decouple layers (e.g., `Repository` trait vs `PostgresRepository` impl).
- Define **DTOs** (Data Transfer Objects) for boundary communication.

**Example (Rust):**
```rust
// domain/src/ports.rs
pub trait UserRepository {
    async fn find_by_id(&self, id: Uuid) -> Result<User>;
}
// This allows testing the Service layer with a MockUserRepository
```

### Step 3: Layered Architecture Definition

Define the responsibility of each layer to ensure separation of concerns:
- **Presentation Layer**: UI, CLI. Talk to Application Layer via Interfaces.
- **Application Layer**: Use Cases. Pure logic orchestration. Mockable dependencies.
- **Domain Layer**: Entities, Rules. No external dependencies.
- **Infrastructure Layer**: DB, API. Implements interfaces defined in Domain/App layers.

### Step 4: Documentation & Continuity Setup

Create the following files immediately:
- `README.md`: Project overview and getting started.
- `ARCHITECTURE.md`: Explained layers and interaction boundaries.
- `task.md`: **The Source of Truth for Progress**.

### Step 5: Implementation Roadmap (Living Plan)

Create a phased plan. **CRITICAL**: The plan must be updated continuously.

- **Phase 1: Skeleton**: Project init, CI, dependency injection setup.
- **Phase 2: Core Domain**: Data models, strictly unit-tested domain logic.
- **Phase 3: Vertical Slice**: One feature end-to-end, validating interfaces.
- **Phase 4: Scale Out**: Remaining features.

## Execution Rules for Agents

1.  **Update Plans First**: Before starting any code task, mark it "In Progress" in `task.md`.
2.  **Update Plans Last**: When a task is done, mark it "Completed" in `task.md` and add notes if needed.
3.  **Context Resumption**: Ensure that reading `README.md` and `task.md` provides 100% context to resume work after a pause.

## Output Template

When finishing the architecture phase, output a **Technical Design Document**:

```markdown
# Technical Design: [Project Name]

## 1. System Overview
[High level description]

## 2. Architecture & Interfaces
[Diagram or Tree]
### Key Interfaces
- `InterfaceName`: [Description] (Enables mocking for TDD)

## 3. Data Model
[Key Entities]

## 4. Implementation Plan (Seed for task.md)
- [ ] Phase 1: Skeleton & CI
- [ ] Phase 2: Domain (TDD)
    - [ ] Define Domain Entities
    - [ ] Unit Test Rules
```








