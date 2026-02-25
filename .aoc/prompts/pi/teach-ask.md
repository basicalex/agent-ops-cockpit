---
description: Ask teach-mode a direct question with clean, answer-only output.
---
Answer the user's teach-mode question directly.

Input:
- Use the `/teach-ask` arguments as the question.
- If no arguments were provided, ask for the question in one short line.

Output contract (strict):
- Return answer-only content.
- Do not include orchestration/meta text such as:
  - "I generated..."
  - "called the task tool..."
  - "task_id..."
  - "returned answer..."
- Do not include placeholders.

Teaching style:
- Keep the response practical, repo-specific, and concise.
- Include file references when making implementation claims.
- If useful, end with a short numbered next-step choice.

Guardrails:
- Read-only exploration by default.
- Do not edit code unless explicitly requested.
- Keep insight logging local-only under `.aoc/insight/` when used.
