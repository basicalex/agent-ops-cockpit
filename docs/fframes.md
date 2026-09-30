# fframes video

[fframes](https://fframes.studio) lets agents build videos with Rust and SVG and render them on the GPU. `aoc-fframes-install` installs the system video libraries, Rust's `cargo-fframes` project generator, and the `fframes-video` agent skill. It updates existing installations on later runs.

The skill lives at `~/.agents/skills/fframes-video/`. Codex, OpenCode, and OMP read that directory; Claude Code uses `~/.claude/skills/fframes-video`, linked to it. The installer downloads the skill from fframes.studio, not the large GitHub repository.

- `aoc-fframes-install`: install or update dependencies and the skill.
- `aoc-fframes-install --check`: report missing pieces without making changes; exit non-zero if incomplete.
- `aoc-fframes-install --smoke`: create, inspect, and render a temporary test video, then remove it.

Set `AOC_FFRAMES_SKIP_DEPS=1` to skip system packages, `AOC_FFRAMES_SKIP_SKILL=1` to skip the skill, or `AOC_CLAUDE_INSTALL_OFFLINE=1` to skip network steps.

Keep a video's project in the repo that owns the video, under `videos/<slug>/`. Marketing videos go in `~/dev/prism`; standalone video projects go in `~/dev/videos/<slug>/`.

Agent loop: plan the timeline, run `inspect`, render a `strip` to check timing, render a `frame` to check composition, use `render --draft` for a quick pass, then `render` the final output. See `~/.agents/skills/fframes-video/SKILL.md` for commands and API details.
