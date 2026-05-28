# 11 — brittle string needles

**Severity**: warning  
**Itemized**: 100

### hv-0772 — `apps/editor/vox-vscode/src/agents/AgentController.ts:150`

**Substring**

```text
complete
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "complete" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: medium

---

### hv-0773 — `apps/editor/vox-vscode/src/agents/AgentController.ts:151`

**Substring**

```text
fail
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "fail" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0774 — `apps/editor/vox-vscode/src/agents/AgentController.ts:152`

**Substring**

```text
start
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "start" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0775 — `apps/editor/vox-vscode/src/agents/AgentController.ts:153`

**Substring**

```text
wait
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "wait" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0776 — `apps/editor/vox-vscode/src/agents/AgentController.ts:159`

**Substring**

```text
build
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "build" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0777 — `apps/editor/vox-vscode/src/agents/AgentController.ts:160`

**Substring**

```text
plan
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "plan" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0778 — `apps/editor/vox-vscode/src/agents/AgentController.ts:161`

**Substring**

```text
debug
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "debug" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0779 — `apps/editor/vox-vscode/src/agents/AgentController.ts:162`

**Substring**

```text
research
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "research" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: medium

---

### hv-0780 — `apps/editor/vox-vscode/src/agents/AgentController.ts:163`

**Substring**

```text
review
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "review" "apps/editor/vox-vscode/src/agents/AgentController.ts"`

**Confidence**: low

---

### hv-0781 — `apps/editor/vox-vscode/src/commands/model.ts:25`

**Substring**

```text
Pull Ollama Model
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Pull Ollama Model" "apps/editor/vox-vscode/src/commands/model.ts"`

**Confidence**: high

---

### hv-0782 — `apps/editor/vox-vscode/src/commands/model.ts:40`

**Substring**

```text
Set API Keys
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Set API Keys" "apps/editor/vox-vscode/src/commands/model.ts"`

**Confidence**: high

---

### hv-0783 — `apps/editor/vox-vscode/src/context/WorkspaceContextEngine.ts:50`

**Substring**

```text
extension-output-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "extension-output-" "apps/editor/vox-vscode/src/context/WorkspaceContextEngine.ts"`

**Confidence**: medium

---

### hv-0784 — `apps/editor/vox-vscode/src/core/VoxMcpClient.ts:151`

**Substring**

```text
Connection
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Connection" "apps/editor/vox-vscode/src/core/VoxMcpClient.ts"`

**Confidence**: medium

---

### hv-0785 — `apps/editor/vox-vscode/src/extension.ts:150`

**Substring**

```text
Yes
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Yes" "apps/editor/vox-vscode/src/extension.ts"`

**Confidence**: low

---

### hv-0786 — `apps/editor/vox-vscode/src/features/linkDiagnostics.ts:27`

**Substring**

```text
http
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http" "apps/editor/vox-vscode/src/features/linkDiagnostics.ts"`

**Confidence**: low

---

### hv-0787 — `apps/editor/vox-vscode/src/features/webArtifactDiagnostics.ts:25`

**Substring**

```text
import type
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "import type" "apps/editor/vox-vscode/src/features/webArtifactDiagnostics.ts"`

**Confidence**: high

---

### hv-0788 — `apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts:28`

**Substring**

```text
..
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF ".." "apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts"`

**Confidence**: low

---

### hv-0789 — `apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts:145`

**Substring**

```text
Yes
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Yes" "apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts"`

**Confidence**: low

---

### hv-0790 — `apps/editor/vox-vscode/src/VisualEditorPanel.ts:109`

**Substring**

```text
http
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http" "apps/editor/vox-vscode/src/VisualEditorPanel.ts"`

**Confidence**: low

---

### hv-0791 — `apps/experimental/visualizer/src/components/AgentFlow.tsx:21`

**Substring**

```text
Failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Failed" "apps/experimental/visualizer/src/components/AgentFlow.tsx"`

**Confidence**: low

---

### hv-0792 — `apps/experimental/visualizer/src/components/AgentFlow.tsx:32`

**Substring**

```text
Failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Failed" "apps/experimental/visualizer/src/components/AgentFlow.tsx"`

**Confidence**: low

---

### hv-0793 — `crates/vox-actor-runtime/src/builtins/tests.rs:53`

**Substring**

```text
hi
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "hi" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: low

---

### hv-0794 — `crates/vox-actor-runtime/src/builtins/tests.rs:85`

**Substring**

```text
invalid params_json
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "invalid params_json" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0795 — `crates/vox-actor-runtime/src/builtins/tests.rs:98`

**Substring**

```text
openclaw worker send failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "openclaw worker send failed" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0796 — `crates/vox-actor-runtime/src/builtins/tests.rs:115`

**Substring**

```text
openclaw worker recv failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "openclaw worker recv failed" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0797 — `crates/vox-actor-runtime/src/builtins/tests.rs:141`

**Substring**

```text
vox-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox-" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: low

---

### hv-0798 — `crates/vox-actor-runtime/src/builtins/tests.rs:173`

**Substring**

```text
hello
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "hello" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: low

---

### hv-0799 — `crates/vox-actor-runtime/src/builtins/tests.rs:196`

**Substring**

```text
a.txt
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "a.txt" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: low

---

### hv-0800 — `crates/vox-actor-runtime/src/builtins/tests.rs:230`

**Substring**

```text
invalid JSON body
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "invalid JSON body" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0801 — `crates/vox-actor-runtime/src/builtins/tests.rs:260`

**Substring**

```text
http worker send failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http worker send failed" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0802 — `crates/vox-actor-runtime/src/builtins/tests.rs:282`

**Substring**

```text
http worker recv failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http worker recv failed" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: high

---

### hv-0803 — `crates/vox-actor-runtime/src/builtins/tests.rs:304`

**Substring**

```text
ok
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "ok" "crates/vox-actor-runtime/src/builtins/tests.rs"`

**Confidence**: low

---

### hv-0804 — `crates/vox-actor-runtime/src/inference_env.rs:233`

**Substring**

```text
cuda
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "cuda" "crates/vox-actor-runtime/src/inference_env.rs"`

**Confidence**: low

---

### hv-0805 — `crates/vox-actor-runtime/src/llm_result.rs:171`

**Substring**

```text
```
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`\`\`" "crates/vox-actor-runtime/src/llm_result.rs"`

**Confidence**: low

---

### hv-0806 — `crates/vox-actor-runtime/src/llm_result.rs:264`

**Substring**

```text
EOF
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "EOF" "crates/vox-actor-runtime/src/llm_result.rs"`

**Confidence**: low

---

### hv-0807 — `crates/vox-actor-runtime/src/llm_result.rs:267`

**Substring**

```text
rate limited
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "rate limited" "crates/vox-actor-runtime/src/llm_result.rs"`

**Confidence**: high

---

### hv-0808 — `crates/vox-actor-runtime/src/llm_result.rs:270`

**Substring**

```text
activity failed
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "activity failed" "crates/vox-actor-runtime/src/llm_result.rs"`

**Confidence**: high

---

### hv-0809 — `crates/vox-actor-runtime/src/llm/chat.rs:46`

**Substring**

```text
chat/completions
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "chat/completions" "crates/vox-actor-runtime/src/llm/chat.rs"`

**Confidence**: medium

---

### hv-0810 — `crates/vox-actor-runtime/src/llm/embed.rs:68`

**Substring**

```text
embeddings
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "embeddings" "crates/vox-actor-runtime/src/llm/embed.rs"`

**Confidence**: medium

---

### hv-0811 — `crates/vox-actor-runtime/src/llm/stream.rs:36`

**Substring**

```text
chat/completions
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "chat/completions" "crates/vox-actor-runtime/src/llm/stream.rs"`

**Confidence**: medium

---

### hv-0812 — `crates/vox-actor-runtime/src/observability.rs:72`

**Substring**

```text
vox-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox-" "crates/vox-actor-runtime/src/observability.rs"`

**Confidence**: low

---

### hv-0813 — `crates/vox-actor-runtime/src/pid.rs:50`

**Substring**

```text
<0.
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "<0." "crates/vox-actor-runtime/src/pid.rs"`

**Confidence**: low

---

### hv-0814 — `crates/vox-actor-runtime/src/prompt_canonical.rs:273`

**Substring**

```text
parser
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "parser" "crates/vox-actor-runtime/src/prompt_canonical.rs"`

**Confidence**: low

---

### hv-0815 — `crates/vox-actor-runtime/src/prompt_canonical.rs:287`

**Substring**

```text
Objectives
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Objectives" "crates/vox-actor-runtime/src/prompt_canonical.rs"`

**Confidence**: medium

---

### hv-0816 — `crates/vox-actor-runtime/src/prompt_canonical.rs:288`

**Substring**

```text
1.
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "1." "crates/vox-actor-runtime/src/prompt_canonical.rs"`

**Confidence**: low

---

### hv-0817 — `crates/vox-arch-check/src/forbidden_patterns.rs:136`

**Substring**

```text
Command::new("git")
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Command::new(\"git\")" "crates/vox-arch-check/src/forbidden_patterns.rs"`

**Confidence**: medium

---

### hv-0818 — `crates/vox-arch-check/src/main.rs:494`

**Substring**

```text
src/
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "src/" "crates/vox-arch-check/src/main.rs"`

**Confidence**: low

---

### hv-0819 — `crates/vox-arch-check/src/main.rs:687`

**Substring**

```text
//!
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "//!" "crates/vox-arch-check/src/main.rs"`

**Confidence**: low

---

### hv-0820 — `crates/vox-arch-check/src/main.rs:862`

**Substring**

```text
## [
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "## [" "crates/vox-arch-check/src/main.rs"`

**Confidence**: medium

---

### hv-0821 — `crates/vox-arch-check/src/main.rs:1069`

**Substring**

```text
target
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "target" "crates/vox-arch-check/src/main.rs"`

**Confidence**: low

---

### hv-0822 — `crates/vox-arch-check/src/main.rs:1086`

**Substring**

```text
target
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "target" "crates/vox-arch-check/src/main.rs"`

**Confidence**: low

---

### hv-0823 — `crates/vox-arch-check/src/main.rs:1087`

**Substring**

```text
my_vendor
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "my_vendor" "crates/vox-arch-check/src/main.rs"`

**Confidence**: medium

---

### hv-0824 — `crates/vox-bounded-fs/src/lib.rs:74`

**Substring**

```text
exceeds scaling policy max_file_bytes_hint
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "exceeds scaling policy max_file_bytes_hint" "crates/vox-bounded-fs/src/lib.rs"`

**Confidence**: high

---

### hv-0825 — `crates/vox-cli-core/src/daemon_ipc/dispatch.rs:220`

**Substring**

```text
http://
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http://" "crates/vox-cli-core/src/daemon_ipc/dispatch.rs"`

**Confidence**: low

---

### hv-0826 — `crates/vox-cli/src/artifact_policy.rs:95`

**Substring**

```text
target-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "target-" "crates/vox-cli/src/artifact_policy.rs"`

**Confidence**: low

---

### hv-0827 — `crates/vox-cli/src/autofix.rs:51`

**Substring**

```text
```
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`\`\`" "crates/vox-cli/src/autofix.rs"`

**Confidence**: low

---

### hv-0828 — `crates/vox-cli/src/autofix.rs:52`

**Substring**

```text
fn 
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "fn " "crates/vox-cli/src/autofix.rs"`

**Confidence**: medium

---

### hv-0829 — `crates/vox-cli/src/command_catalog.rs:441`

**Substring**

```text
vox build
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox build" "crates/vox-cli/src/command_catalog.rs"`

**Confidence**: medium

---

### hv-0830 — `crates/vox-cli/src/command_catalog.rs:442`

**Substring**

```text
recommended
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "recommended" "crates/vox-cli/src/command_catalog.rs"`

**Confidence**: medium

---

### hv-0831 — `crates/vox-cli/src/command_catalog.rs:455`

**Substring**

```text
shell
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "shell" "crates/vox-cli/src/command_catalog.rs"`

**Confidence**: low

---

### hv-0832 — `crates/vox-cli/src/commands/agent.rs:110`

**Substring**

```text
description
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "description" "crates/vox-cli/src/commands/agent.rs"`

**Confidence**: medium

---

### hv-0833 — `crates/vox-cli/src/commands/build.rs:452`

**Substring**

```text
./
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "./" "crates/vox-cli/src/commands/build.rs"`

**Confidence**: low

---

### hv-0834 — `crates/vox-cli/src/commands/build.rs:513`

**Substring**

```text
Missing.tsx
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "Missing.tsx" "crates/vox-cli/src/commands/build.rs"`

**Confidence**: medium

---

### hv-0835 — `crates/vox-cli/src/commands/bundle.rs:65`

**Substring**

```text
wasi
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "wasi" "crates/vox-cli/src/commands/bundle.rs"`

**Confidence**: low

---

### hv-0836 — `crates/vox-cli/src/commands/bundle.rs:174`

**Substring**

```text
windows
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "windows" "crates/vox-cli/src/commands/bundle.rs"`

**Confidence**: low

---

### hv-0837 — `crates/vox-cli/src/commands/bundle.rs:353`

**Substring**

```text
windows
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "windows" "crates/vox-cli/src/commands/bundle.rs"`

**Confidence**: low

---

### hv-0838 — `crates/vox-cli/src/commands/ci/agentskills_compliance.rs:88`

**Substring**

```text
.skill.md
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF ".skill.md" "crates/vox-cli/src/commands/ci/agentskills_compliance.rs"`

**Confidence**: medium

---

### hv-0839 — `crates/vox-cli/src/commands/ci/agentskills_compliance.rs:115`

**Substring**

```text
vox-plugin-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox-plugin-" "crates/vox-cli/src/commands/ci/agentskills_compliance.rs"`

**Confidence**: medium

---

### hv-0840 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:33`

**Substring**

```text
evaluate_interruption
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "evaluate_interruption" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: medium

---

### hv-0841 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:34`

**Substring**

```text
record_attention_event
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "record_attention_event" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: medium

---

### hv-0842 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:35`

**Substring**

```text
AttentionEventType::
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "AttentionEventType::" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: medium

---

### hv-0843 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:36`

**Substring**

```text
interruption_policy.rs
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "interruption_policy.rs" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: medium

---

### hv-0844 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:37`

**Substring**

```text
lib.rs
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "lib.rs" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: low

---

### hv-0845 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:38`

**Substring**

```text
mod.rs
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "mod.rs" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: low

---

### hv-0846 — `crates/vox-cli/src/commands/ci/attention_ledger_parity.rs:39`

**Substring**

```text
attention_policy.rs
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "attention_policy.rs" "crates/vox-cli/src/commands/ci/attention_ledger_parity.rs"`

**Confidence**: medium

---

### hv-0847 — `crates/vox-cli/src/commands/ci/build_timings.rs:315`

**Substring**

```text
vox-
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox-" "crates/vox-cli/src/commands/ci/build_timings.rs"`

**Confidence**: low

---

### hv-0848 — `crates/vox-cli/src/commands/ci/canonical_docs.rs:91`

**Substring**

```text
docs/src/archive/
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "docs/src/archive/" "crates/vox-cli/src/commands/ci/canonical_docs.rs"`

**Confidence**: medium

---

### hv-0849 — `crates/vox-cli/src/commands/ci/canonical_docs.rs:102`

**Substring**

```text

category:
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "
category:" "crates/vox-cli/src/commands/ci/canonical_docs.rs"`

**Confidence**: high

---

### hv-0850 — `crates/vox-cli/src/commands/ci/canonical_docs.rs:145`

**Substring**

```text

status: legacy
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "
status: legacy" "crates/vox-cli/src/commands/ci/canonical_docs.rs"`

**Confidence**: high

---

### hv-0851 — `crates/vox-cli/src/commands/ci/check_links.rs:166`

**Substring**

```text
```
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`\`\`" "crates/vox-cli/src/commands/ci/check_links.rs"`

**Confidence**: low

---

### hv-0852 — `crates/vox-cli/src/commands/ci/check_links.rs:178`

**Substring**

```text
http
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "http" "crates/vox-cli/src/commands/ci/check_links.rs"`

**Confidence**: low

---

### hv-0853 — `crates/vox-cli/src/commands/ci/check_links.rs:180`

**Substring**

```text
mailto:
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "mailto:" "crates/vox-cli/src/commands/ci/check_links.rs"`

**Confidence**: low

---

### hv-0854 — `crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs:23`

**Substring**

```text
=>
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "=>" "crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs"`

**Confidence**: low

---

### hv-0855 — `crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs:57`

**Substring**

```text
//
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "//" "crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs"`

**Confidence**: low

---

### hv-0856 — `crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs:60`

**Substring**

```text
=>
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "=>" "crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs"`

**Confidence**: low

---

### hv-0857 — `crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs:97`

**Substring**

```text
vox_
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox_" "crates/vox-cli/src/commands/ci/command_compliance/mcp_wiring.rs"`

**Confidence**: low

---

### hv-0858 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:81`

**Substring**

```text
`manifest
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`manifest" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0859 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:126`

**Substring**

```text
vox_config_get
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox_config_get" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0860 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:127`

**Substring**

```text
vox_get_config
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox_get_config" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0861 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:128`

**Substring**

```text
vox_other
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox_other" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0862 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:140`

**Substring**

```text
vox_indented_only
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "vox_indented_only" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0863 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:147`

**Substring**

```text
`import
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`import" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: low

---

### hv-0864 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:148`

**Substring**

```text
verify
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "verify" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: low

---

### hv-0865 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:155`

**Substring**

```text
`manifest
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "\`manifest" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: medium

---

### hv-0866 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:223`

**Substring**

```text
visible_alias = "orchestrator"
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "visible_alias = \"orchestrator\"" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: high

---

### hv-0867 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:228`

**Substring**

```text
visible_alias = "clavis"
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "visible_alias = \"clavis\"" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: high

---

### hv-0868 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:233`

**Substring**

```text
visible_alias = "speech"
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "visible_alias = \"speech\"" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: high

---

### hv-0869 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:249`

**Substring**

```text
fabrica
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "fabrica" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: low

---

### hv-0870 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:253`

**Substring**

```text
secrets
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "secrets" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: low

---

### hv-0871 — `crates/vox-cli/src/commands/ci/command_compliance/tests.rs:258`

**Substring**

```text
clavis
```

**Why it matters**: Case-sensitive substring checks often fail on real user or OS input.

**Fix** (normalize-input-or-casefold): Normalize (trim + lower) before compare, or use str::eq_ignore_ascii_case / unicase as appropriate.

**Verify**: `rg -nF "clavis" "crates/vox-cli/src/commands/ci/command_compliance/tests.rs"`

**Confidence**: low

---

