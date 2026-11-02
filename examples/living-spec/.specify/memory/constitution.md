# Northstar Helpdesk Constitution

## Core Principles

### I. Small deterministic domain
Keep the sample a Rust library using the standard library. No network services, authentication, persistence or UI are in scope.

### II. Specification before business change
Document and approve business behavior before changing it. Preserve a living spec and distinguish an implementation defect from a new business rule.

### III. Evidence through tests
Execute deterministic tests. New behavior needs a regression; previously accepted behavior remains protected unless the specification changes explicitly.

## Development Workflow
Spec Kit artifacts guide implementation. Review spec and plan, implement tasks, execute cargo test, and review code. External publication is separate from local delivery.

## Governance
Changes to FR-003 require approval and a versioned specification update. Retain old artifacts and source history.

**Version**: 1.0.0 | **Ratified**: 2026-10-29 | **Last Amended**: 2026-10-29
