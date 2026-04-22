# Mind Background Reliability Checklist

Short exit criteria for calling project-scoped `aoc-mind` a dependable background runtime.

## Target operating model

`aoc-mind` should run quietly in the background per project, keep ingest/query/export state moving forward, and stay out of the way while agents work on code.

## Checklist

### 1. Canonical ownership
- `aoc-mind` owns project-local ingest, retrieval, context-pack, provenance, observer/finalize preparation, and runtime health semantics.
- Hosts (`aoc-agent-wrap-rs`, Pi extensions, Mission Control) consume Mind-owned APIs instead of re-implementing policy.

### 2. Standalone session viability
- Mind works without Pulse being present.
- A normal Pi/AOC coding session can ingest, query, observer-run, and finalize through standalone service surfaces.
- Project-scoped store selection is deterministic and does not drift to global/shared state.

### 3. Background liveness honesty
- `aoc-mind-service status` reports whether the background service is actually:
  - `running`
  - `degraded`
  - `stale`
  - `inactive` / `idle`
  - `cold`
- Expired leases or stale heartbeats are surfaced explicitly, not implied away by leftover snapshot files.
- Pi `/mind-status` exposes the same canonical service-state judgment.

### 4. Cheap continuous operation
- Normal ingest/sync stays bounded and project-local.
- Queue depth and detached work stay visible without requiring full scans.
- Background status checks do not require heavy startup or large log parsing.

### 5. Recovery behavior
- Stale detached jobs reconcile cleanly on startup.
- Cancelled jobs stay cancelled.
- Spawn failures fall back deterministically instead of silently stalling work.
- Finalize/export remains idempotent under repeat calls and drain pressure.

### 6. Query reliability
- Retrieval returns honest fallback signals when no good hit exists.
- Context-pack and provenance queries share canonical Mind-owned parsing/compilation.
- Compatibility hosts do not bypass canonical request semantics.

### 7. Validation bar
- Focused crate tests cover the canonical ownership seams in `aoc-mind`.
- Existing wrapper compatibility tests remain green.
- `docs/mind-runtime-validation.md` commands still pass for live smoke + hardening coverage.

## Current hardening focus

The current lightweight reliability hardening target is **background liveness honesty**:

- add canonical stale/degraded service-state detection in `aoc-mind`
- surface it through `aoc-mind-service status`
- surface it in Pi `/mind-status`

That does not make Mind “bulletproof,” but it closes one of the highest-value gaps for always-on background use: avoiding false confidence from stale lease/health artifacts.
