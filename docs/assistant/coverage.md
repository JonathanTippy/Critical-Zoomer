# Living llvm-cov baseline (assistant)

**Source of truth for V2V Coverage claims:** `coverage-baseline.txt` in this
directory (committed). Regenerate with:

```bash
taskset -c $(nproc | awk '{s=int($1/4); e=int($1*3/4)-1; print s"-"e}') scripts/coverage.sh
```

HTML detail stays under `target/llvm-cov/html` (not committed). A copy of the
text summary is also written to `Trash/coverage-baseline.txt` for history.

Do not treat Trash-only numbers as current after a tip advances — re-run and
commit the living file.
