# Regression mode

Use for proving a bug fix or preventing a changed surface from breaking adjacent behavior.

Checklist:
- state the previous failure or baseline expectation
- run the narrowest reproduction first
- verify the fixed path and one adjacent negative/edge path
- add or recommend a durable test only if it fits the existing test stack
- report coverage gaps explicitly
