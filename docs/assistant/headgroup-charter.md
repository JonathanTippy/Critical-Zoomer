# Headgroup / shadergroup charter (post–v0.0.9)

**Standing rule.** After the v0.0.9 restore (`e6a0560`), work lives in the
**workgroup**. Headgroup and shadergroup changes are allowed only when they are:

1. **Location bar / goto** (coords parse, readout, apply), or
2. **HUD telemetry** (mode / ref / gear / IPS / PPS / rates), or
3. An **explicit, documented** shade/display fix with its own Tracey rule and
   pinned tests.

Anything else that slips into colorer / escaper / window sampling is a charter
violation until justified or reverted.

## Diff vs `e6a0560` (audit 2026-08-09)

| Area | Δ | Charter? |
|---|---|---|
| `headgroup/window/coords.rs` | +647 (new module) | **Yes** — location / goto |
| `headgroup/window/rolling.rs` | +76 (PPS / rates) | **Yes** — HUD |
| `headgroup/window/mod.rs` | HUD wiring | **Yes** — HUD |
| `headgroup/window/snip.rs` | +72 in-app PPM | Borderline — faux-user / assistant visual; keep |
| `headgroup/window/inputs.rs` | small | Review if zoom-debt salvage |
| `headgroup/window/sampling.rs` | small | Review |
| `shadergroup/colorer/color.rs` | +319 filament / period edges | **Documented feature** — shade rules; needs headed verify |
| `shadergroup/escaper.rs` | escape-continues / ring | Tied to shade; keep with shade rules |

## Enforcement

- Quality ticks: re-run `git diff e6a0560..HEAD --stat -- src/assemblies/headgroup/ src/assemblies/shadergroup/` and update this table.
- New headgroup/shadergroup edits outside the three buckets require a one-paragraph
  note in `issue-stack.md` **before** the change lands.
- Do not “fix” charter drift with `#[ignore]` or soft bars
  (`docs/assistant/quality-doctrine.md`).
