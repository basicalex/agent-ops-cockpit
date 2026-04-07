# Managed Zellij Top Bar

AOC ships a managed `zjstatus`-based top bar plugin as:

- repo source: `vendor/zjstatus-aoc/`
- repo wasm: `zellij/plugins/zjstatus-aoc.wasm`
- installed wasm: `~/.config/zellij/plugins/zjstatus-aoc.wasm`

## Repair / install

```bash
aoc-zellij-plugin install
```

`aoc-init` also repairs this managed plugin automatically.

## Inspect status

```bash
aoc-zellij-plugin status
```

Useful fields include:

- `dest_wasm`
- `cache_wasm`
- `reference_wasm`
- `dest_matches_reference`
- the SHA256 lines for each copy

## Verify

```bash
aoc-zellij-plugin verify
```

This exits non-zero if the installed plugin is missing or diverges from the managed bundled copy.

## Rebuild the tracked wasm from vendored source

```bash
bash scripts/zellij/rebuild-managed-plugin.sh
```

## Verify the tracked wasm matches the vendored source

```bash
bash scripts/zellij/verify-managed-plugin.sh
```

CI also runs the managed plugin smoke/build path to keep the source snapshot and committed wasm in sync.

## Tab project metadata

The grouped tab bar now supports explicit project metadata via Zellij pipe messages.

Official AOC layouts call this automatically at pane startup:

```bash
aoc-tab-metadata sync
```

You can also inspect or override the current tab metadata manually:

```bash
aoc-tab-metadata status
aoc-tab-metadata set --project-key voyager
aoc-tab-metadata set --project-key voyager --tab-name Voyager
```

This improves grouped-tab accuracy beyond plain tab-name inference, especially when tab labels are short or customized.

## Layout hygiene

AOC now treats `aoc` as the single official managed general-purpose layout.

Legacy managed layout names such as these should be removed and not reused:

- `unstat`
- `minimal`
- `aoc-zjstatus-single`
- `aoc-zjstatus-test`
- `aoc.hybrid`

AOC prunes these stale managed layout files during install. Dedicated Mission Control still uses its own explicit layout path and launcher.
