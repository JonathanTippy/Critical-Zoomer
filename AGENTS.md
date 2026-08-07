# Critical-Zoomer — working agreement for agents

This repo's value is a small set of *proven* mechanisms, restored and verified.
Do not redesign them by accident. Read these before editing.

## Authoritative docs (read in this order when touching the workgroup)

1. `docs/assistant/design/workgroup-virtues.md` — the normative design and *why*
   each mechanism exists.
2. `docs/assistant/tracey/craftsmanship-rules.md` — the binding rules
   (`r[cz.craft.*]`), one per golden mechanism, each with acceptance criteria and
   pinned tests.
3. `docs/assistant/issue-stack.md` — live bugs, design gaps, and the standing
   rule that new work must not re-break the v0.0.9 invariants.

## The invariants (summary — full text in the rules file)

`.cursor/rules/critical-zoomer-invariants.mdc` is the always-on version. In
brief: no unbounded calls (use `BoutCap`), one live target (the `LiveTarget`
struct), whole-snapshot publishes, pivot two-message order, provisional never
final (the `Delivery` enum), small interruptible bouts.

## Enforcement layers

Three layers, cheapest-first:

1. **Types** — `BoutCap` (no unbounded call), `Delivery` (provisional cannot set
   `delivered`), `push_delivery` (buffer slot + flag atomically, `#[must_use]`),
   `LiveTarget` (one live target structural). Compile-time; cannot regress.
2. **The pre-edit hook** — `.cursor/hooks.json` runs
   `.cursor/hooks/workgroup-rules.sh` on every `Write`/`Edit`/`StrReplace` into
   `screen_worker/` or `colorer/`; it injects that file's rule summaries as agent
   context at the moment of the edit. Fails open.
3. **Tests + tracey audit** — catch what types and the hook miss.

## Two rules that prevent most regressions

- **Prefer a type over a comment** for any invariant. If the bad state can be
  made unrepresentable, make it unrepresentable (`BoutCap` is the pattern).
- **The change protocol:** a deliberate redesign updates the tracey rule, the
  virtues doc, and the pinned test together, in one change. A failing rule is a
  regression until the developer says otherwise.

## Verify

Run the full test suite after workgroup/colorer edits. Keep tracey links intact
(every `r[impl ...]` resolves to a rule; every rule's tests exist).

For any change that can affect rendered output, also follow
`docs/assistant/manual-testing.md`. This is assistant-owned work: build the
current release, run it under the isolated Xvfb harness, capture PNGs, and
inspect them directly. Never capture the developer's desktop and never hand the
procedure to the developer.
