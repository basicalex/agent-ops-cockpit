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
- `aoc-theme sync`

## Scope model
- Global themes live at `~/.config/zellij/themes/<name>.kdl`.
- Project scope is deprecated and no longer supported.

## What gets themed
- Zellij core theme (`theme "..."`)
- Zellij status surfaces (`zjstatus`) in shipped AOC layouts
- AOC Pulse (`aoc-mission-control`) via `AOC_THEME_*` palette env
- Yazi via generated `~/.config/yazi/theme.toml`

## Recommended workflow
1. Run `aoc-theme list` to check existing names.
2. Create the theme with `aoc-theme init`.
3. Edit palette values in the generated file.
4. If inside Zellij, apply live with `aoc-theme apply --name <theme>`.
5. Persist for future launches with `aoc-theme set-default --name <theme>`.
6. Run `aoc-theme sync` if configs were edited manually outside `aoc-theme`.

## Guardrails
- Theme names must match `^[a-z0-9]+(-[a-z0-9]+)*$`.
- `--scope project` is rejected; migrate old project themes into `~/.config/zellij/themes/`.
- Do not overwrite existing theme files unless replacement was explicitly requested.
- Live apply requires an active Zellij pane (`zellij options --theme ...`).
