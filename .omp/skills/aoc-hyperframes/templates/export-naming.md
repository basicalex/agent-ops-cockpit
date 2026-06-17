# HyperFrames Export Naming

## Pattern
`project-audience-channel-duration-concept-vN.ext`

## Examples
- `voyager-business-meta-15s-qr-demo-v1.mp4`
- `voyager-business-reel-6s-multilingual-hook-v1.mp4`
- `voyager-landing-hero-loop-v1.webm`

## Output Folders
- Brand/system renders: `hyperframes/renders/brand/`
- Ads: `hyperframes/renders/ads/<campaign>/`
- Social: `hyperframes/renders/social/<campaign>/`
- Landing/site loops: `hyperframes/renders/landing/`
- Delivery bundles: `hyperframes/renders/exports/`

## Rules
- Never overwrite final exports silently.
- Increment `vN` for meaningful changes.
- Document source composition and command in retrospective or render notes.
