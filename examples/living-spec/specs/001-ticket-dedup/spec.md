# Feature Specification: Ticket duplicate merge V1

**Feature Branch**: `001-ticket-dedup`  
**Created**: 2026-10-29  
**Status**: Existing baseline

## User Scenarios & Testing

### User Story 1 - Merge occurrences (Priority: P1)

An operator retains the older ticket when a duplicate arrives.

**Independent Test**: `cargo test` exercises `merge_duplicate`.

**Acceptance Scenarios**:
1. Matching fingerprints: sum the occurrence counters.
2. Older existing ticket: preserve its id and fingerprint.
3. Existing P3, incoming P1: V1 deliberately preserves P3.
4. Different fingerprints: fail the domain precondition with an assertion.

### Edge Cases

The caller supplies the older ticket as `existing`. Priorities are prevalidated by the caller. There are no SLA, escalation-group, HTTP, auth or persistence fields in this small library.

## Requirements

- **FR-001**: `merge_duplicate` MUST require exact matching fingerprints.
- **FR-002**: The merged ticket MUST retain the existing id and fingerprint.
- **FR-003**: V1 MUST preserve the existing priority, even if the incoming ticket has greater urgency.
- **FR-004**: The merged ticket MUST sum occurrences.
- **FR-005**: Tests MUST cover occurrences, identity, priority retention and mismatched fingerprints.

### Key Entities

**Ticket**: id, fingerprint, numeric priority and occurrences, as implemented in src/lib.rs.

## Success Criteria

- **SC-001**: All baseline scenarios pass deterministic Rust tests without external dependencies.
- **SC-002**: A change to FR-003 requires approval, a living-spec update and a new regression before implementation.

## Clarifications

V1 is intentional. incident.json requests V2, not a code fix relative to V1. V2 must choose minimum numeric priority while preserving FR-001, FR-002 and FR-004.
