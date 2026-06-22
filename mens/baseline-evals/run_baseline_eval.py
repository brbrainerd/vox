"""
Vox Mens baseline function-calling eval.

Runs a representative set of function-calling test cases per spoke against a
local Ollama endpoint and writes bfcl_results.json for each spoke so that
`vox mens baseline` can record the pre-training baseline.

Output per spoke: mens/baseline-evals/base-qwen3-8b/<spoke>/bfcl_results.json
  {"accuracy": 0.xx, "total": NNN}

Usage:
  python mens/baseline-evals/run_baseline_eval.py
"""

import json
import os
import sys
import urllib.request
import urllib.error

OLLAMA_BASE = "http://localhost:11434/v1"
MODEL = "qwen3:8b"
OUT_DIR = os.path.join(os.path.dirname(__file__), "base-qwen3-8b")

# ---------------------------------------------------------------------------
# Minimal OpenAI-compat chat + tool-call helper
# ---------------------------------------------------------------------------

def chat(messages, tools=None, temperature=0.0):
    body = {
        "model": MODEL,
        "messages": messages,
        "temperature": temperature,
        "stream": False,
    }
    if tools:
        body["tools"] = tools
        body["tool_choice"] = "auto"
    data = json.dumps(body).encode()
    req = urllib.request.Request(
        f"{OLLAMA_BASE}/chat/completions",
        data=data,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        print(f"  HTTP {e.code}: {e.read().decode()[:200]}", file=sys.stderr)
        return None
    except Exception as e:
        print(f"  Error: {e}", file=sys.stderr)
        return None


def extract_tool_call(response):
    """Return (function_name, arguments_dict) or (None, None)."""
    if not response:
        return None, None
    msg = response.get("choices", [{}])[0].get("message", {})
    calls = msg.get("tool_calls") or []
    if not calls:
        # Some models inline the call in content as JSON
        content = msg.get("content", "")
        if content and "{" in content:
            try:
                obj = json.loads(content[content.index("{"):content.rindex("}")+1])
                if "name" in obj:
                    return obj["name"], obj.get("arguments", obj.get("parameters", {}))
            except Exception:
                pass
        return None, None
    call = calls[0].get("function", {})
    name = call.get("name")
    args = call.get("arguments", {})
    if isinstance(args, str):
        try:
            args = json.loads(args)
        except Exception:
            args = {}
    return name, args


# ---------------------------------------------------------------------------
# Grading helpers
# ---------------------------------------------------------------------------

def grade_name(got_name, expected_name):
    return got_name == expected_name if got_name else False


def grade_args(got_args, required_keys):
    """True if all required keys are present and non-empty in got_args."""
    if not got_args:
        return False
    return all(k in got_args and got_args[k] not in (None, "", []) for k in required_keys)


def run_cases(cases, spoke):
    passed = 0
    for i, case in enumerate(cases):
        print(f"  [{spoke}] case {i+1}/{len(cases)}: {case['label']}", end=" ", flush=True)
        resp = chat(case["messages"], tools=case.get("tools"))
        name, args = extract_tool_call(resp)
        name_ok = grade_name(name, case["expected_fn"])
        args_ok = grade_args(args, case.get("required_args", []))
        ok = name_ok and args_ok
        passed += int(ok)
        print("PASS" if ok else f"FAIL (fn={name!r}, args={args})")
    return passed, len(cases)


# ---------------------------------------------------------------------------
# Spoke: vox-lang
# Tests function-calling in the context of Vox language tooling
# ---------------------------------------------------------------------------

VOX_LANG_TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "compile_vox",
            "description": "Compile a Vox source file and return diagnostics.",
            "parameters": {
                "type": "object",
                "properties": {
                    "source_path": {"type": "string", "description": "Path to the .vox source file"},
                    "target": {"type": "string", "enum": ["native", "wasm", "interp"], "description": "Compilation target"},
                },
                "required": ["source_path", "target"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_vox_script",
            "description": "Execute a Vox script in the interpreter.",
            "parameters": {
                "type": "object",
                "properties": {
                    "script": {"type": "string", "description": "Vox source code to run"},
                    "timeout_ms": {"type": "integer", "description": "Execution timeout in milliseconds"},
                },
                "required": ["script"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "format_vox",
            "description": "Format Vox source code according to style rules.",
            "parameters": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Vox source code to format"},
                },
                "required": ["source"],
            },
        },
    },
]

VOX_LANG_CASES = [
    {
        "label": "compile native",
        "messages": [{"role": "user", "content": "Compile the file src/main.vox targeting native output."}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "compile_vox",
        "required_args": ["source_path", "target"],
    },
    {
        "label": "compile wasm",
        "messages": [{"role": "user", "content": "Build src/app.vox for WebAssembly deployment."}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "compile_vox",
        "required_args": ["source_path", "target"],
    },
    {
        "label": "run script",
        "messages": [{"role": "user", "content": 'Run this Vox snippet: let x = 42; print(x)'}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "run_vox_script",
        "required_args": ["script"],
    },
    {
        "label": "format code",
        "messages": [{"role": "user", "content": "Format this Vox code: fn foo(){let x=1;x}"}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "format_vox",
        "required_args": ["source"],
    },
    {
        "label": "compile interp",
        "messages": [{"role": "user", "content": "Run tests/eval.vox through the interpreter."}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "compile_vox",
        "required_args": ["source_path", "target"],
    },
    {
        "label": "run with timeout",
        "messages": [{"role": "user", "content": "Execute this script with a 5 second limit: fn main() { 1 + 1 }"}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "run_vox_script",
        "required_args": ["script"],
    },
    {
        "label": "format before commit",
        "messages": [{"role": "user", "content": "Before I commit, please format: pub fn add(a:i64,b:i64)->i64{a+b}"}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "format_vox",
        "required_args": ["source"],
    },
    {
        "label": "compile check only",
        "messages": [{"role": "user", "content": "Check if lib/core.vox compiles without running it, native target."}],
        "tools": VOX_LANG_TOOLS,
        "expected_fn": "compile_vox",
        "required_args": ["source_path", "target"],
    },
]

# ---------------------------------------------------------------------------
# Spoke: rust
# Tests Rust code review / analysis function calls
# ---------------------------------------------------------------------------

RUST_TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "run_clippy",
            "description": "Run Clippy lint checks on a Rust crate.",
            "parameters": {
                "type": "object",
                "properties": {
                    "crate_path": {"type": "string", "description": "Path to the Rust crate root"},
                    "fix": {"type": "boolean", "description": "Apply automatic fixes", "default": False},
                },
                "required": ["crate_path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_cargo_test",
            "description": "Run the test suite for a Rust crate.",
            "parameters": {
                "type": "object",
                "properties": {
                    "crate_path": {"type": "string", "description": "Path to the Rust crate root"},
                    "filter": {"type": "string", "description": "Optional test name filter"},
                },
                "required": ["crate_path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "cargo_build",
            "description": "Build a Rust crate.",
            "parameters": {
                "type": "object",
                "properties": {
                    "crate_path": {"type": "string", "description": "Path to the Rust crate root"},
                    "release": {"type": "boolean", "description": "Build in release mode", "default": False},
                },
                "required": ["crate_path"],
            },
        },
    },
]

RUST_CASES = [
    {
        "label": "clippy check",
        "messages": [{"role": "user", "content": "Run Clippy on the crate at crates/vox-compiler."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_clippy",
        "required_args": ["crate_path"],
    },
    {
        "label": "run tests",
        "messages": [{"role": "user", "content": "Run all unit tests in crates/vox-populi."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_cargo_test",
        "required_args": ["crate_path"],
    },
    {
        "label": "build release",
        "messages": [{"role": "user", "content": "Build crates/vox-cli in release mode."}],
        "tools": RUST_TOOLS,
        "expected_fn": "cargo_build",
        "required_args": ["crate_path"],
    },
    {
        "label": "clippy autofix",
        "messages": [{"role": "user", "content": "Auto-fix Clippy warnings in crates/vox-db."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_clippy",
        "required_args": ["crate_path"],
    },
    {
        "label": "run single test",
        "messages": [{"role": "user", "content": "Run just the test named 'beat_base_clear' in crates/vox-ml-cli."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_cargo_test",
        "required_args": ["crate_path"],
    },
    {
        "label": "debug build",
        "messages": [{"role": "user", "content": "Build crates/vox-orchestrator-mcp in debug mode to check it compiles."}],
        "tools": RUST_TOOLS,
        "expected_fn": "cargo_build",
        "required_args": ["crate_path"],
    },
    {
        "label": "clippy all features",
        "messages": [{"role": "user", "content": "Lint crates/vox-corpus with Clippy."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_clippy",
        "required_args": ["crate_path"],
    },
    {
        "label": "test with filter",
        "messages": [{"role": "user", "content": "Run tests matching 'wilson' in crates/vox-ml-cli."}],
        "tools": RUST_TOOLS,
        "expected_fn": "run_cargo_test",
        "required_args": ["crate_path"],
    },
]

# ---------------------------------------------------------------------------
# Spoke: tool-selection
# Tests the model's ability to pick the RIGHT tool from a larger set
# ---------------------------------------------------------------------------

TOOL_SEL_TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "search_web",
            "description": "Search the web for current information.",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]},
        },
    },
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a file from the local filesystem.",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]},
        },
    },
    {
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Write content to a file.",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}, "required": ["path", "content"]},
        },
    },
    {
        "type": "function",
        "function": {
            "name": "execute_shell",
            "description": "Execute a shell command.",
            "parameters": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]},
        },
    },
    {
        "type": "function",
        "function": {
            "name": "query_database",
            "description": "Run a SQL query against the application database.",
            "parameters": {"type": "object", "properties": {"sql": {"type": "string"}}, "required": ["sql"]},
        },
    },
    {
        "type": "function",
        "function": {
            "name": "send_email",
            "description": "Send an email to a recipient.",
            "parameters": {"type": "object", "properties": {"to": {"type": "string"}, "subject": {"type": "string"}, "body": {"type": "string"}}, "required": ["to", "subject", "body"]},
        },
    },
]

TOOL_SEL_CASES = [
    {"label": "read local config", "messages": [{"role": "user", "content": "Read the contents of config/settings.yaml."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "read_file", "required_args": ["path"]},
    {"label": "web search", "messages": [{"role": "user", "content": "Find the latest Rust release notes."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "search_web", "required_args": ["query"]},
    {"label": "write output", "messages": [{"role": "user", "content": "Save the analysis results to output/report.txt."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "write_file", "required_args": ["path", "content"]},
    {"label": "shell command", "messages": [{"role": "user", "content": "Run 'cargo test' in the project directory."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "execute_shell", "required_args": ["command"]},
    {"label": "db query", "messages": [{"role": "user", "content": "Count the number of users registered today."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "query_database", "required_args": ["sql"]},
    {"label": "send notification", "messages": [{"role": "user", "content": "Email alice@example.com with subject 'Build passed' and body 'All tests green.'"}], "tools": TOOL_SEL_TOOLS, "expected_fn": "send_email", "required_args": ["to", "subject", "body"]},
    {"label": "read log file", "messages": [{"role": "user", "content": "Show me what's in logs/error.log."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "read_file", "required_args": ["path"]},
    {"label": "search for docs", "messages": [{"role": "user", "content": "Look up the OpenAI function calling documentation."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "search_web", "required_args": ["query"]},
    {"label": "list db records", "messages": [{"role": "user", "content": "Query all active sessions from the sessions table."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "query_database", "required_args": ["sql"]},
    {"label": "write config", "messages": [{"role": "user", "content": "Write the updated configuration to config/prod.yaml."}], "tools": TOOL_SEL_TOOLS, "expected_fn": "write_file", "required_args": ["path", "content"]},
]

# ---------------------------------------------------------------------------
# Spoke: argument-generation
# Tests correct argument extraction / generation for function calls
# ---------------------------------------------------------------------------

ARG_GEN_TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "create_user",
            "description": "Create a new user account.",
            "parameters": {
                "type": "object",
                "properties": {
                    "username": {"type": "string"},
                    "email": {"type": "string"},
                    "role": {"type": "string", "enum": ["admin", "editor", "viewer"]},
                },
                "required": ["username", "email", "role"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "schedule_job",
            "description": "Schedule a recurring background job.",
            "parameters": {
                "type": "object",
                "properties": {
                    "job_name": {"type": "string"},
                    "cron_expr": {"type": "string", "description": "Standard cron expression"},
                    "enabled": {"type": "boolean"},
                },
                "required": ["job_name", "cron_expr", "enabled"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "resize_image",
            "description": "Resize an image to specified dimensions.",
            "parameters": {
                "type": "object",
                "properties": {
                    "input_path": {"type": "string"},
                    "width": {"type": "integer"},
                    "height": {"type": "integer"},
                    "output_path": {"type": "string"},
                },
                "required": ["input_path", "width", "height", "output_path"],
            },
        },
    },
]

ARG_GEN_CASES = [
    {"label": "create admin user", "messages": [{"role": "user", "content": "Create a new admin user: username=bob, email=bob@example.com."}], "tools": ARG_GEN_TOOLS, "expected_fn": "create_user", "required_args": ["username", "email", "role"]},
    {"label": "create viewer", "messages": [{"role": "user", "content": "Add a viewer account for carol@acme.org, username carol."}], "tools": ARG_GEN_TOOLS, "expected_fn": "create_user", "required_args": ["username", "email"]},
    {"label": "schedule daily job", "messages": [{"role": "user", "content": "Schedule the 'cleanup' job to run every day at midnight."}], "tools": ARG_GEN_TOOLS, "expected_fn": "schedule_job", "required_args": ["job_name", "cron_expr"]},
    {"label": "schedule weekly disabled", "messages": [{"role": "user", "content": "Add a weekly report job (every Sunday at 8am) but keep it disabled for now."}], "tools": ARG_GEN_TOOLS, "expected_fn": "schedule_job", "required_args": ["job_name", "cron_expr", "enabled"]},
    {"label": "resize 800x600", "messages": [{"role": "user", "content": "Resize photos/original.jpg to 800x600 and save as photos/thumb.jpg."}], "tools": ARG_GEN_TOOLS, "expected_fn": "resize_image", "required_args": ["input_path", "width", "height", "output_path"]},
    {"label": "resize square", "messages": [{"role": "user", "content": "Make a 256×256 thumbnail of assets/logo.png, output to assets/logo_sm.png."}], "tools": ARG_GEN_TOOLS, "expected_fn": "resize_image", "required_args": ["input_path", "width", "height", "output_path"]},
    {"label": "create editor", "messages": [{"role": "user", "content": "Register dave@corp.com as an editor, username: dave."}], "tools": ARG_GEN_TOOLS, "expected_fn": "create_user", "required_args": ["username", "email", "role"]},
    {"label": "schedule hourly", "messages": [{"role": "user", "content": "Enable an hourly metrics export job called 'metrics-export'."}], "tools": ARG_GEN_TOOLS, "expected_fn": "schedule_job", "required_args": ["job_name", "cron_expr"]},
]

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

SPOKES = {
    "vox-lang": VOX_LANG_CASES,
    "rust": RUST_CASES,
    "tool-selection": TOOL_SEL_CASES,
    "argument-generation": ARG_GEN_CASES,
}

def main():
    # Quick connectivity check
    try:
        req = urllib.request.Request(f"{OLLAMA_BASE}/models", method="GET")
        with urllib.request.urlopen(req, timeout=5):
            pass
    except Exception as e:
        print(f"ERROR: Cannot reach Ollama at {OLLAMA_BASE}: {e}", file=sys.stderr)
        sys.exit(1)

    print(f"Running baseline eval against {MODEL} at {OLLAMA_BASE}\n")

    all_passed = 0
    all_total = 0

    for spoke, cases in SPOKES.items():
        print(f"=== Spoke: {spoke} ===")
        passed, total = run_cases(cases, spoke)
        print(f"  Result: {passed}/{total} ({passed/total*100:.1f}%)\n")
        all_passed += passed
        all_total += total

        spoke_dir = os.path.join(OUT_DIR, spoke)
        os.makedirs(spoke_dir, exist_ok=True)
        result = {"accuracy": passed / total, "total": total}
        with open(os.path.join(spoke_dir, "bfcl_results.json"), "w") as f:
            json.dump(result, f, indent=2)

    print(f"=== Overall: {all_passed}/{all_total} ({all_passed/all_total*100:.1f}%) ===")
    print(f"\nResults written to: {OUT_DIR}/<spoke>/bfcl_results.json")


if __name__ == "__main__":
    main()
