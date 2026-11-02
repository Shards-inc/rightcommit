# Northstar Helpdesk

Small Rust repository for real Spec Kit runs. No external dependencies.
`cargo test` executes the domain tests.

Feature request: implement a deterministic ticket deduplication service that
groups matching fingerprints, retains the oldest id, sums occurrences and
documents the V1 rule: preserve the oldest ticket's priority.

The follow-up incident in `incident.json` requests a business change to V2:
the retained ticket must inherit the highest urgency (lowest numeric priority).
Treat the old behavior as conforming to V1; update the living specification
before changing implementation and adding a regression test.
