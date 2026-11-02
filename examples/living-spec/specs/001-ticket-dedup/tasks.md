# Tasks: Ticket duplicate merge V1

**Input**: spec.md and plan.md in this directory.

## Existing baseline

- [x] T001 Define Ticket and merge_duplicate in src/lib.rs.
- [x] T002 Require exact matching fingerprints in src/lib.rs.
- [x] T003 Preserve existing id, fingerprint and priority in src/lib.rs.
- [x] T004 Sum occurrences and test the result in src/lib.rs.
- [x] T005 Test identity retention and intentional V1 priority retention in src/lib.rs.
- [x] T006 Test the mismatched-fingerprint precondition in src/lib.rs.

## Future change

incident.json is not implemented by this baseline. Update the living spec to V2, regenerate plan/tasks through Spec Kit, and add the P1-into-P3 regression before changing FR-003 behavior.
