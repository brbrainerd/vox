---
description: Run TOESTUB to check the codebase for architectural anti-patterns.
---

1. Run the TOESTUB self-apply script:
// turbo
```pwsh
.\toestub_self_apply.ps1
```

2. Review the findings:
- If `arch/god_object` is flagged, split the large file into sub-modules.
- If `arch/sprawl` is flagged, rename generic files or group files into feature directories.
- If `arch/organization` is flagged, extract definitions from `lib.rs` into dedicated modules.
- If `arch/unwired_module` is flagged, ensure the module is declared in `lib.rs` or `mod.rs`.

3. Re-run the check after refactoring to ensure the violations are resolved.
