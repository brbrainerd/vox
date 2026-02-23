# Improved AI "Continue" Prompt

```xml
<instructions>
Continue executing all remaining steps from our current plan in priority order, including any optional steps.

<imperatives>
- TAKE ACTION: Execute actions via tool calls (terminal, browser, search) rather than describing them. Do not summarize your intentions beforehand unless absolutely necessary.
- NO PLACEHOLDERS: Implement all code completely inline — no stubs, TODOs, FIXMEs, placeholders, or magic values. Assume this is the final edit.
- DO NOT EXPLAIN: Minimize conversational filler and avoid long explanations of code changes. Just output the changes.
</imperatives>

<execution_rules>
- PREVENT DUPLICATION: Before creating any new file, section, or component, explicitly confirm no equivalent already exists.
- INCREMENTAL WIRING: Wire new code into existing systems immediately before moving on to new branches or systems.
- VERIFY KNOWLEDGE: Use web search to verify best practices before performing destructive operations, complex refactors, or anything relying on post-2024 information.
</execution_rules>

<testing_and_verification>
- TEST DEPENDENT SYSTEMS: Validate foundational systems and their dependencies *before* building systems that depend on them. Do not defer verification to the end.
- TARGET CONTENT: Run targeted tests and linting against changed files only — avoid running the entire test suite unless requested.
- WRITE PERSISTENT TESTS: Expand the test suite maintaining the testing hourglass (many unit tests, fewer integration, fewest E2E). Balance coverage gains against execution time.
</testing_and_verification>

<tool_preferences>
- NATIVE FIRST: Prefer native API tool calls over shell commands when both can accomplish the same task (e.g. use `grep_search` instead of bash `grep`).
- FALLBACK ONLY: Fall back to terminal commands only when native tool calls are insufficient.
- SEARCH SMARTLY: Use browser/search tools strictly when you need current (2025–2026) information or when verifying the safety of destructive operations.
</tool_preferences>

<quality_gates>
- CONTEXT AWARENESS: Apply only the instructions above that are strictly relevant to the current task.
- EFFICIENCY: Avoid unnecessary tool calls, tests, or searches that do not directly advance the plan.
- ERROR HANDLING: If you encounter an error (compilation, test failure), attempt to resolve it immediately inside a single atomic loop rather than reporting it and stopping.
</quality_gates>
</instructions>
```
