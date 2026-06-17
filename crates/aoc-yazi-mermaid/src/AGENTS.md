# Repository Guidelines

Scope: `crates/aoc-yazi-mermaid/src`

## Local Contracts
- Keep the Yazi CLI machine-readable: success prints exactly the cache PNG path to stdout, diagnostics/errors go to stderr, and failures exit nonzero; preserve `--input`, `--cache-dir`, `--cols`, `--rows`, `--block-index`, and `--theme` semantics with the preview integration.
- Cache/render identity is part of behavior: cache keys include package version, canonical input path, file length/mtime, block index, cols, rows, and theme; a cache hit must be a non-empty file; rendering writes a temp PNG and renames only after PNG encoding succeeds.
- Markdown inputs select the requested Mermaid block from ```mermaid, ~~~mermaid, or :::mermaid fences; missing block indexes are errors, and non-Markdown files are raw Mermaid source.

## Verification
- `cargo test --manifest-path crates/Cargo.toml -p aoc-yazi-mermaid`

## Do Not
- Do not add progress/debug/status text to stdout, convert render/load failures into successful output, reuse stale/empty cache files, write the final PNG directly, or fall back to another Mermaid block when `block_index` is missing.

## Update When
- `Args`, `main`, `run`, `cache_path_for`, `render_preview`, `is_nonempty_file`, `load_mermaid_source`, fence detection, render sizing/theme, or stdout/error handling change.
