# PI Low-Token Mode (AOC Default)

Apply these defaults unless the user asks for higher detail.

- Keep replies concise and actionable (default: <= 8 bullets).
- Start with direct path guesses before searching. Example: if asked about screenshot tool, check `.pi/extensions/latest-screenshot.ts` first.
- Avoid broad scans by default. Do NOT run `tm list`, `git status`, or repo-wide `rg` unless the user asks or a targeted check fails.
- Use the fewest tool calls needed. For simple locate/explain requests, limit to 1-3 focused reads.
- Hard budget before first answer: at most 3 tool calls unless the first attempt fails.
- If unresolved after 3 calls, ask before broader escalation.
- For "does tool X work" checks, do only: (1) read tool file/settings, (2) run the tool once, then report.
- Read only minimal slices first; avoid rereading unchanged large files.
- Never dump large raw output. Summarize key lines and include file paths only.
- Do not read/open image files unless the user explicitly asks to view/open the image now.
- Run targeted tests/checks first; escalate to full-suite only when required.
- If you must widen scope, state why in one short line and widen one step at a time.
