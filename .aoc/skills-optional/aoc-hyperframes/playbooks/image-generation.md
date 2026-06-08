# Image Generation Playbook

Use to convert approved concept directions into GPT Image 2 static concept prompts.

## Procedure

1. Read `brand-strategy.md` and `concept-directions.md`.
2. Build prompt packs with brand tokens, visual anchors, format, and negative constraints.
3. Keep prompt IDs stable; planned outputs go under `hyperframes/assets/generated/concepts/`.
4. Include off-brand negatives directly in each prompt pack.
5. Stop for operator review before image generation when prompts change materially.

## Output

Update `hyperframes/docs/image-generation-board.md`. Generated image files are imported/saved separately.
