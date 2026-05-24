# Test preset core

Use this preset after implementation or when the user asks to verify, test, QA, inspect a preview, reproduce a bug, or validate that a built surface actually works.

Active testing posture:
- map the changed files/routes/components to the user journeys they affect
- read relevant implementation narrowly before testing when needed
- use browser automation for UI flows, screenshots, forms, navigation, responsive checks, console errors, and visual regressions
- use architecture/design reasoning to validate route structure, state flow, information architecture, and edge cases
- run targeted lint/type/test/build commands only as needed for confidence
- report what passed, what failed, what was not tested, and the next highest-value verification

Do not confuse testing with redesign. Recommend design changes only when they block usability, accessibility, or task completion.
