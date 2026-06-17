# Repository Guidelines

Scope: `crates/aoc-segment-routing/src`

## Local Contracts
- SegmentRouter::compute_auto_route must prefer a non-empty active Taskmaster tag mapped by tag_to_segment over heuristics, emit RouteOrigin::Taskmaster, use Taskmaster confidence, and keep taskmaster_tag_map...source=context_state provenance.
- Heuristic routing must use default_uncertain_segment for low-confidence or ambiguous top candidates; uncertain_fallback keeps useful secondary candidates and includes the normalized default_global_segment fallback when absent.
- Manual overrides must reject empty patch_id/primary segment, normalize and dedupe segments case-insensitively, cap secondaries, preserve prior auto route candidates when possible, set ManualOverride/overridden_by, and include override_patch plus base provenance.

## Verification
- `cargo test -p aoc-segment-routing --lib`
