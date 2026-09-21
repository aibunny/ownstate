## Why

- Evolve the existing implementation toward the root architecture/requirements
  without replacing the working evidence-to-context vertical slice.
- The audit identified event-derived classification downgrade and stale ranked-ID
  retrieval gaps that should close before a richer institutional domain ships.

## What Changes

- Add a manual DISCOVER → PLAN → EXECUTE → VERIFY → ITERATE contract, fixed
  verification runner, truthful check receipts, gap analysis and next-cycle state.
- Derive candidate/version classification from cited events; revalidate event
  provenance within promotion; add an additive SQL view/insert guard to prevent
  new unsafe evidence links and hide unsafe legacy history on read.
- Recheck tenant/project/classification/ACTIVE/current validity when loading
  ranked content; exercise lifecycle changes during query embedding.
- Correct present-vs-target documentation and local test instructions.

## Impact

- No new API surface, canonical content rewrite, optional infrastructure,
  hosted provider requirement, commit or push. Migration 0007 is additive.
- Four workspace gates, real PostgreSQL fresh/upgrade tests, loop-runner failure
  tests, and independent review are required to complete this batch.
- This batch establishes the implementation loop; broader root requirements
  remain explicitly tracked for dependency-ordered subsequent cycles.
