# Image Review Board

Concept source: `hyperframes/docs/image-generation-board.md`

## Image decisions

| Image ID | Source path | Concept ID | Decision | Reason | Approved source path |
| --- | --- | --- | --- | --- | --- |
| IMG-001 | `hyperframes/assets/generated/concepts/IMG-001.png` | C01 | draft | | `hyperframes/assets/generated/approved/IMG-001.png` |

Allowed decisions: `draft`, `approved`, `rejected`, `needs-revision`.

## Region markings for extraction

Use normalized coordinates when possible. Store crops/overlays under `hyperframes/assets/generated/sections/` only when useful; metadata is enough when the region can be described precisely.

| Region ID | Image ID | Region/crop path | Bounds | Element description | SVG target | Specialist |
| --- | --- | --- | --- | --- | --- | --- |
| REG-001 | IMG-001 | `hyperframes/assets/generated/sections/REG-001.png` | x=, y=, w=, h= | | `hyperframes/assets/generated/svg/REG-001.svg` | svg-asset |

## Operator approval

- [ ] Approved images copied or referenced in `assets/generated/approved/`
- [ ] Rejected images keep rejection reason
- [ ] Regions are specific enough for SVG extraction
- [ ] No unapproved region proceeds to SVG work
