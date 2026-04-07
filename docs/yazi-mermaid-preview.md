# Yazi Mermaid Preview

AOC includes a Mermaid preview path for Yazi built around a small Rust helper, a Yazi plugin, and a cached-PNG workflow.

This document describes the current architecture, the live UX, the limitations we intentionally accepted, and the next likely upgrade path.

## Current status

The current system is the agreed v1:

- Yazi matches `*.mmd` and `*.mermaid`
- a helper renders Mermaid to a cached PNG
- Yazi previews that PNG through its normal image pipeline
- `Alt+Enter` renders the current Mermaid file and opens the PNG externally as a safe fallback

This is working and is the recommended path today.

## Why this architecture exists

We evaluated direct terminal drawing approaches such as [`mermaidcat`](https://github.com/zhengbuqian/mermaidcat), which renders Mermaid and then writes terminal-native image output directly.

That model was useful as a reference, but it was not chosen for Yazi because Yazi already owns:

- preview pane placement
- redraw timing
- clearing and repaint behavior
- terminal backend selection

For Yazi, the more robust split is:

1. Mermaid source -> rendered image artifact
2. let Yazi display that artifact using its own preview backend

This avoids brittle direct escape-sequence drawing inside Yazi's preview lifecycle.

## Final architecture

### Render path

```text
.mmd / .mermaid
  -> aoc-yazi-mermaid helper
  -> cached PNG in ~/.cache/aoc/yazi-mermaid
  -> Yazi image preview
```

### Open path

```text
Alt+Enter in Yazi
  -> aoc-mermaid-open plugin
  -> aoc-yazi-mermaid-open wrapper
  -> aoc-yazi-mermaid helper
  -> cached PNG
  -> aoc-open-file / system opener
```

## Components

- Wrapper: `bin/aoc-yazi-mermaid`
- Native helper binary: `aoc-yazi-mermaid-native`
- Rust helper source: `crates/aoc-yazi-mermaid`
- Open wrapper: `bin/aoc-yazi-mermaid-open`
- Yazi preview plugin: `yazi/plugins/aoc-mermaid.yazi`
- Yazi open plugin: `yazi/plugins/aoc-mermaid-open.yazi`
- Yazi rules: `yazi/yazi.toml`
- Yazi keybinding: `yazi/keymap.toml`
- Installer integration: `install.sh`

## Renderer choice

The current helper is implemented in Rust and uses:

- `mermaid-rs-renderer`

We chose Rust over Go because:

- the repo already has a Rust workspace
- the renderer candidate already existed in Rust
- it keeps the v1 dependency surface smaller than a Node/`mmdc` path

## Supported inputs

Yazi preview rules currently target:

- `*.mmd`
- `*.mermaid`

The helper can also extract Mermaid blocks from Markdown, but Markdown files are **not** bound to this previewer in Yazi because we do not want to hijack normal Markdown preview.

## Install and rollout

This feature is intended to work through the normal AOC install flow:

```bash
./install.sh
aoc-init /path/to/repo
```

The installer builds and installs the native helper so the feature can be used from other repos after standard AOC setup.

## Cache behavior

Rendered PNGs are stored under:

- `${XDG_CACHE_HOME:-$HOME/.cache}/aoc/yazi-mermaid`

These are runtime/view artifacts, not canonical project outputs.

### Cache key inputs

The cache key currently includes:

- helper package version
- canonical input path
- file length
- modified time
- selected Mermaid block index
- preview `cols`
- preview `rows`
- theme

This means different pane sizes and theme choices produce different cached files.

### Current cache policy

The current policy is:

- keep preview/open artifacts in the cache
- do **not** write them next to source files by default

If a stable export layer is added later, it should be something separate such as:

- `diagrams/rendered/`
- `.aoc/map/diagrams/rendered/`

That export layer should be treated as a durable project artifact, not as a replacement for runtime cache.

## Preview quality expectations

The current preview is intended for:

- browsing
- orientation
- quick visual inspection
- simple to moderately dense diagrams

It is **not** intended to be a perfect replacement for:

- a browser render
- a large dedicated diagram viewer
- dense-diagram close inspection in a small Yazi pane

## Fidelity vs web/site output

AOC currently uses two Mermaid backends:

### Yazi/open PNG path

- `mermaid-rs-renderer`

### AOC Map / microsite path

- Mermaid JS in the browser

So exact parity is **not expected**.

Remaining differences may include:

- layout spacing
- edge routing
- text wrapping differences
- theme/styling drift

That is an accepted tradeoff in the current v1.

## Important rendering conclusion

The earlier "broken/useless" renders were not purely a renderer-backend problem.

A significant part of the bad output came from our helper/render path and was fixed. After those fixes, the current renders are considered:

- readable
- usable for preview/browsing
- still not guaranteed to match Mermaid JS exactly

## Yazi UX

### Automatic preview

When Yazi hovers a Mermaid file, AOC tries to render a cached PNG and lets Yazi preview it normally.

### Manual open fallback

When inline preview quality or backend support is limited, use:

- `Alt+Enter`

This renders the Mermaid file and opens the PNG externally.

For non-Mermaid files, this action falls back to normal AOC open behavior.

### Open render size

You can tune the render size used by the open action with:

```bash
AOC_YAZI_MERMAID_OPEN_COLS=180
AOC_YAZI_MERMAID_OPEN_ROWS=60
```

## Terminal and multiplexer reality

The main blocker for ideal inline preview is not Mermaid rendering itself.

It is the terminal/multiplexer stack.

### Important current limitation

True inline image preview inside:

- Yazi
- running under Zellij
- in current released Zellij

is still limited by Zellij image protocol handling.

### Practical outcome

- outside restrictive multiplexer paths, Yazi image preview can work well
- inside current released Zellij, inline image behavior still depends on terminal/backend behavior
- Kitty/kitten is the preferred path for inline graph/image preview
- the safe fallback is `Alt+Enter` external open

That fallback is intentional and is currently the recommended escape hatch.

## Environment inspection

Use:

```bash
aoc-yazi-preview detect
```

Look for fields such as:

- `aoc_yazi_mermaid=yes`
- `effective_mode=...`
- `terminal_family=...`
- `native_backend=kitty|kitten|none`
- `in_zellij=yes|no`

## Helper environment variables

### Theme selection

```bash
AOC_YAZI_MERMAID_THEME=auto
AOC_YAZI_MERMAID_THEME=dark
AOC_YAZI_MERMAID_THEME=light
```

`auto` is the default.

Optional hint:

```bash
AOC_YAZI_MERMAID_AUTO_THEME=light
AOC_YAZI_MERMAID_AUTO_THEME=dark
```

### Cache directory override

```bash
AOC_YAZI_MERMAID_CACHE_DIR=/custom/cache/path
```

### Markdown Mermaid block selection

```bash
AOC_YAZI_MERMAID_BLOCK_INDEX=0
```

### Wrapper binary override

```bash
AOC_YAZI_MERMAID_BIN=/path/to/custom/helper
```

## Manual test

```bash
aoc-yazi-mermaid --input path/to/diagram.mmd --cols 100 --rows 35
```

This prints the cached PNG path on stdout.

## Daily-use summary

Use the system like this:

1. launch Yazi normally through your AOC flow
2. hover a `.mmd` or `.mermaid` file
3. if inline preview is good enough, keep browsing
4. if inline preview is limited or blocked, press `Alt+Enter`

That is the current intended UX.

## Current consensus

The current project consensus is:

- keep the cached-PNG architecture
- keep the Rust helper for v1
- keep Mermaid preview rules scoped to `.mmd` and `.mermaid`
- keep preview/open artifacts in cache by default
- keep `Alt+Enter` as the safe Mermaid open shortcut
- do not overwrite default Yazi `Shift+O` behavior
- accept that Yazi/open output will not exactly match Mermaid JS site renders
- treat a future project-local `rendered/` directory as an optional export layer, not as the runtime cache

## Known limitations

- exact Mermaid JS parity is not guaranteed
- cache invalidation is metadata-based today, not full content-hash based
- dense diagrams may still be hard to inspect inside a small preview pane
- current released Zellij remains the main blocker for ideal inline image behavior

## Likely future improvements

The most likely next improvements are:

1. **content-hash cache keys** for stronger cache correctness
2. **optional project-local export path** such as `diagrams/rendered/`
3. **Mermaid-JS-based render path** for closer parity with site output if fidelity becomes more important than simplicity

## When to consider switching backends

If you need preview/open output to match the browser render much more closely, the next step is not more terminal escape-sequence work.

The next step is to replace or complement the current renderer with a Mermaid-JS-backed path such as `mmdc`.

That would likely improve parity, but it would increase runtime/dependency complexity.
