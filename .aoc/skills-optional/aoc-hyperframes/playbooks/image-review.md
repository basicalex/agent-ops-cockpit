# Image Review Playbook

Use when reviewing generated/imported concept images with the operator.

## Procedure

1. Register every concept image path and prompt/concept ID.
2. Mark decisions as draft, approved, rejected, or needs-revision with reasons.
3. Copy/reference approved images under `assets/generated/approved/` when policy allows.
4. Mark extraction regions with coordinates, crop path, element description, and SVG target.
5. Do not dispatch SVG extraction for unapproved regions.

## Output

Update `hyperframes/docs/image-review-board.md`. Region records are the contract for `svg-asset` OMP subagents.
