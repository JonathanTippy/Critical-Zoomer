# New unit: Window controls

No authoritative unit file beyond thin headgroup notes. Non-authoritative.

## Window / viewport (UD-UI-WIN-1) — inferred

- Default 800×480; do not restore customized size on launch.
- One viewport covers the window; resizes with window.
- Headgroup frame cap 60fps.

## Movement (UD-UI-MOVE-1) — inferred

WASD, arrows, LMB drag. Pan uses elapsed time (headgroup supplement).

## Zoom (UD-UI-ZOOM-1) — inferred

- Wheel: origin at mouse; 2× per bump; debt-gap so ticks don’t skip/backlog.
- Shift / Space: origin at center; slower (~5 bumps/s).
- Scroll up = zoom in.

## Home (UD-UI-HOME-1) — inferred

Home button top-right → viewport to `0 + 0i` (center).

## Coordinates (UD-UI-COORD-1) — inferred + D-UI-1

- Read-only selectable location field (center) + Copy.
- Editable goto field: accept all likely forms — two numbers space/comma or plus-with-i; parens/brackets/braces/extra spaces; rich forms like `(5i + 6)` → `(6 + 5i)`.
- Apply greys when empty or invalid.
- Apply **stays enabled** when field equals current location (D-UI-1).
- Apply moves center to parsed location; field not cleared.

## Settings chrome (UD-UI-SET-1) — inferred

Gear top-right opens secondary settings window; widgets + drag-and-drop layer order.

## Off-screen arrows (UD-UI-OFF-1) — inferred

Use auth headgroup geometric thresholds on r=2 circle; show red arrows when off / mostly-off / too-small / mostly-too-small.

## Drag-rezoom anchor (UD-UI-DRAG-1) — inferred

Store full-IntExp drag origin so zoom-out then zoom-in returns to the grabbed point.
