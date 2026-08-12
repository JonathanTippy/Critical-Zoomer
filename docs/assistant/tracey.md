# Tracey (assistant ops)

Spec-link tool for this repo. Rules live in `docs/assistant/tracey/*.md`.
Config: `.config/tracey/config.styx`.

## Install (2026-08-12)

`cargo install tracey` **fails** on rustc 1.97 (`roam-types` E0053/E0277/E0308).
Use the **prebuilt** binary (crate README):

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/bearcove/tracey/releases/latest/download/tracey-installer.sh | sh
```

Or tarball `tracey-x86_64-unknown-linux-gnu.tar.xz` from
`https://github.com/bearcove/tracey/releases` into `~/.cargo/bin`.
This machine: **tracey 1.4.0**.

## CLI

There is **no** `tracey validate` subcommand. Use:

```bash
tracey query validate
tracey query status
```

Bacon job `jobs.tracey` already calls `tracey query validate`.

## Config that actually loads rules

Do **not** point `include` at `docs/requirements.md` / `docs/design/*.md` —
those paths are empty/missing and Tracey reports “no requirement definitions /
cannot infer marker prefix.” Live includes are the eight files under
`docs/assistant/tracey/`.

`test_include` must **not** be all of `src/**/*.rs` (that flags every `r[impl]`
as ImplInTestFile). Dedicated test trees only (`craftsmanship_tests`).

Keep a **single** current version header per rule (`+2` not a `+1` stub plus
`+2`). Dual headers made current `+2` refs look unknown.

In tests, use `r[verify …]`, not bare `r[…]` or `verifies r[…]` (bare = impl).

## Snapshot (2026-08-12, after config repair)

`tracey query validate`: **0 errors** (link/config hygiene only).

`tracey query status`: **55/136** with impl refs, **58/136** with verify refs,
**81 uncovered**. Uncovered is **remaining coverage debt**, not a closed bar.
Do not treat validate-clean as “the spec is fully implemented.”
