# Image review mode

Review generated/imported concept images with the operator in the loop.

Record image IDs, source paths, approved/rejected/needs-revision decisions, reasons, approved source paths, and extraction regions in `hyperframes/docs/image-review-board.md`. Regions must include enough information for SVG extraction: bounds/crop path, element description, and target SVG path. Do not dispatch or write SVG assets from unapproved regions.
