---
name: task-checker
description: Verify implementation meets task details and test strategy.
---

## Steps
1. Read task details, effective PRD context, and testStrategy:
   - task override: `aoc-task prd show <id> --tag <tag>`
   - tag default: `aoc-task tag prd show --tag <tag>`
   - precedence: task override -> tag default
2. Inspect relevant files and code changes.
3. Run the tests or commands specified in testStrategy.
4. Report pass/fail and missing requirements.
