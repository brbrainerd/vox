You are deciding the single cheapest enforceable fix that would have prevented
a cluster of related, token-wasting commits in a software project.

You are given a cluster: a group of commits that share a waste category, a
suggested remediation kind, and a primary crate. For the cluster as a whole
(not commit-by-commit) decide ONE authoritative remediation.

Choose an artifact_form from the ALLOWED set you are given (the set excludes
VoxScript unless the host says the authoring model is Vox-capable):
- AgentsMdRule: a one-paragraph rule in AGENTS.md would make agents skip this
- CodeAuditDetector: a vox-code-audit lint detector would catch this at write time
- ArchRule: a vox-arch-check / layers.toml rule would prevent this structurally
- CiGate: a CI contract entry or a test/example fixture would fail on this
- VoxScript: a small `vox run` script would have done the mechanical work in one commit
- CorpusNegativeExample: a MENS fine-tuning negative example would discourage it
- None: the cluster is legitimate work needing no structural fix

Then DRAFT the actual artifact body in the chosen form. Make it concrete and
correct for its target surface — real YAML for CiGate, a real rule spec for
CodeAuditDetector, a real markdown paragraph for AgentsMdRule, etc. Do NOT
draft Vox source unless VoxScript is in the allowed set.

Return one JSON object: { artifact_form, confidence (0..1), synthesized_fix_summary,
drafted_body, form_rationale }.

NEVER mention authors, emails, or blame. Base your judgment only on the diffs
and rationales shown.
