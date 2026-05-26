# These rules apply to every task in this project unless explicitly overridden.

Bias: caution over speed on non-trivial work.

## Rule 1 - Think Before Coding

State assumptions explicitly.
Inspect the codebase before asking the user when the answer is locally discoverable.
Ask only when the gap is material, risky, or not discoverable from local context.

## Rule 2 - Simplicity First

Write the minimum code that solves the problem.
Do not add speculative abstractions or generalize single-use behavior.

## Rule 3 - Surgical Changes

Touch only what the task requires.
Match existing style and patterns.
Do not refactor adjacent code unless the task or correctness requires it.

## Rule 4 - Goal-Driven Execution

Define concrete success criteria before substantial work.
Execute until the result is verified or a real blocker is surfaced.

## Rule 5 - Use Code for Deterministic Answers

Use the model for judgment, summarization, drafting, and tradeoffs.
Use code, scripts, and local inspection for routing, lookups, retries, and deterministic transforms.
If the codebase can answer, prefer the codebase.

## Rule 6 - Token Discipline

Keep context focused and task-scoped.
Read only the files needed for the current goal.
If context is getting large, summarize the current state before continuing.

## Rule 7 - Surface Conflicts Clearly

If patterns or requirements conflict, do not average them together.
Pick the more recent, more local, or more tested path when a choice is required.
Explain the choice and flag the conflict for cleanup.

## Rule 8 - Read Before You Write

Before changing code, inspect the relevant exports, immediate callers, and shared utilities.
If existing structure seems intentional but unclear, investigate locally first, then ask if risk remains.

## Rule 9 - Tests Should Verify Intent

Tests should capture why behavior matters, not only what output happened once.
If business logic can change without failing the test, the test is too weak.

## Rule 10 - Checkpoint After Significant Steps

Be able to summarize what changed, what was verified, and what remains.
Do not keep building on a state you cannot describe clearly.

## Rule 11 - Follow Local Conventions

Conformance beats personal taste inside an existing codebase.
If a convention is harmful, surface it explicitly instead of silently forking from it.

## Rule 12 - Fail Loud

Do not claim completion if anything material was skipped, deferred, or unverified.
Do not say tests passed if relevant tests were not run.
Surface uncertainty instead of hiding it.
