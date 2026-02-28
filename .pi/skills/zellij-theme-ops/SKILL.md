---
name: zellij-theme-ops
description: Create and manage global Zellij themes for AOC.
---

## When to use
- You need a new custom Zellij theme for all projects (global scope).
- You need to apply or persist a theme change quickly during an active session.

## Commands
- `aoc-theme tui`
- `aoc-theme presets list`
- `aoc-theme presets install --all`
- `aoc-theme init --name <theme>`
- `aoc-theme list`
- `aoc-theme apply --name <theme>`
- `aoc-theme set-default --name <theme>`
- `aoc-theme activate --name <theme>`
- `aoc-theme sync`

## Scope model
- Global themes live at `~/.config/zellij/themes/<name>.kdl`.
- Project scope is deprecated and no longer supported.

## What gets themed
- Zellij core theme (`theme "..."`)
- Zellij status surfaces (`zjstatus`) in shipped AOC layouts
- AOC Pulse (`aoc-mission-control`) via `AOC_THEME_*` palette env
- Yazi via generated `~/.config/yazi/theme.toml`
- Pi via generated `~/.pi/agent/themes/aoc-live.json` (`theme: aoc-live`) for live palette sync during AOC theme apply/sync/activate
- Pi extension map respects AOC lock (`AOC_PI_THEME_LOCKED=1`) so extension defaults do not override active AOC theme

## Live preview behavior (AOC Control Theme Manager)
- Open with `Alt+c` -> Theme manager.
- In Presets/Custom lists: `j/k` previews themes live.
- `Enter` selects the fallback theme.
- `a` opens action menu (apply/set-default/install actions).
- Leaving the preview list (`Esc`) restores the selected fallback theme.

## Recommended workflow
1. Run `aoc-theme list` to check existing names.
2. Create the theme with `aoc-theme init`.
3. Edit palette values in the generated file.
4. Prefer one-shot activation with `aoc-theme activate --name <theme>` (live apply + default + artifact sync).
5. Use `aoc-theme apply --name <theme>` only for temporary live preview/apply.
6. Run `aoc-theme sync` if configs were edited manually outside `aoc-theme`.

## Guardrails
- Theme names must match `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `--scope project` is rejected; migrate old project themes into `~/.config/zellij/themes/`.
- Do not overwrite existing theme files unless replacement was explicitly requested.
- Live apply requires an active Zellij pane (`zellij options --theme ...`).
