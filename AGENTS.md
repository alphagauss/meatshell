# AGENTS.md

This file provides guidance to Code/Codex when working with code in this repository.

## Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them; don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken unless the current phase explicitly asks for that refactor.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it; don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the current phase or user request.

## Code Map Discipline

Before any code-related add, delete, modify, or search, read `docs/code-map.md` first.

If a change adds, removes, renames, moves, or rewires a file, function, callback, struct, enum, trait, module, Slint component, UI property, or cross-file dependency, update `docs/code-map.md` in the same change.

Use the code map as the first place to orient yourself before falling back to broad repository-wide searches.

## Independent Maintenance

This repository is now maintained independently. Upstream compatibility is no longer a primary goal.

When implementing features:

- Prefer simple, direct changes over upstream-compatibility scaffolding.
- Refactor when it directly reduces complexity for the current phase or clearly prepares for an already planned phase.
- Do not preserve upstream merge friendliness at the cost of local maintainability.
- Do not keep obsolete fork/upstream assumptions in new code or documentation.
- Avoid broad rewrites unless the current structure blocks the task.
- Keep each change scoped to the current phase.
- Prefer clear local ownership of modules over excessive indirection.
- If a planned change requires invasive edits, state why before editing and keep the changed surface as small as practical.

This project should evolve toward a maintainable standalone terminal manager, not a patch stack on top of the original fork.

## Phase-Driven Execution

Most work in this repository should follow `plan.md`.

For each phase:

- Complete only the current phase's acceptance criteria.
- Do not implement future phases early.
- If you discover a dependency on a future phase, record it as a TODO or note instead of silently expanding the scope.
- Keep each phase buildable and runnable after completion.
- Prefer one focused branch per phase.

For multi-step tasks, state a brief plan:

```text
[Step] -> verify: [check]
[Step] -> verify: [check]
[Step] -> verify: [check]
```

## Goal-Driven Execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

- "Add validation" -> "Write tests for invalid inputs, then make them pass."
- "Fix the bug" -> "Write a test that reproduces it, then make it pass."
- "Refactor X" -> "Ensure tests pass before and after."

Strong success criteria let you loop independently. Weak criteria such as "make it work" require constant clarification.

## Verification Requirements

After each implementation phase, run:

```text
cargo fmt
cargo check
```

If tests exist or the change affects logic that has tests, also run:

```text
cargo test
```

For UI, terminal, SSH, SFTP, file transfer, and tunnel changes, include manual test notes in the final response.

A final response after implementation should include:

1. What files changed.
2. Why those files changed.
3. What was intentionally not implemented.
4. Verification results.
5. Manual test suggestions, when relevant.

## Terminal Engine Safety

When changing terminal rendering, terminal parsing, keyboard input, mouse input, resize behavior, or terminal engine selection:

- Keep the legacy terminal path available unless the phase explicitly removes it.
- Do not make the experimental engine the only path until it has passed basic shell and TUI checks.
- Avoid leaking third-party terminal engine internals into `app.rs` or Slint UI models.
- Preserve ordinary text selection unless the task explicitly changes selection behavior.
- Do not break existing SSH input/output while experimenting with a new engine.

## Connection and Background Task Safety

When changing SSH, SFTP, file transfer, tunnel, or reconnection behavior:

- Every spawned background task must have a clear stop/cancel path.
- UI state must not claim that a connection, transfer, or tunnel is stopped while its worker is still running.
- Reconnect loops must use backoff and must stop when the owning session/rule/window is closed.
- Tunnel failure must not break the main terminal session.
- Closing a file transfer window must not close the main terminal session unless explicitly requested.

## Configuration Safety

When adding persistent configuration:

- Prefer small, explicit config files over broad migration frameworks.
- Do not rewrite existing user config unless necessary.
- If a new config file is introduced, document its purpose and ownership.
- If compatibility logic is required, keep it simple and local to the config loader.

## Documentation Discipline

Update documentation in the same change when behavior changes.

At minimum, update:

- `docs/code-map.md` for code structure, symbols, callbacks, modules, and cross-file dependencies.
- `README.md` or `docs/usage.md` when user-visible behavior changes.
- `plan.md` only when the project plan itself changes.

These guidelines are working if there are fewer unnecessary diffs, fewer rewrites caused by overcomplication, and fewer accidental scope expansions across phases.