# Crate API: vox-py

## Internal Developer Guide: PyO3 Bound API Patterns

The `vox-py` crate has been upgraded to PyO3 0.22+, which deprecates the old unconstrained lifetime API in favor of the new `Bound<'py, T>` "Bound API" patterns. This provides better memory safety and prevents GIL-related lifetime issues.

When making modifications or adding new bindings in `vox-py`, follow these required API patterns:

### Object Conversion
- **DO NOT USE**: `.into_py(py)` (deprecated for `Py<T>` and similar)
- **USE**: `.to_object(py)` when converting values to `PyObject` or `PyAny`

### Creating Python Collections
- **DO NOT USE**: `PyList::new(py, ...)` or `PyDict::new(py)`
- **USE**: `PyList::new_bound(py, ...)` and `PyDict::new_bound(py)`
- **USE**: `PyTuple::new_bound(py, ...)`

### Working with Python Dictionaries
When setting items on a Python dictionary, you must use the Bound API:
```rust
// Old, deprecated way:
// dict.set_item("key", value).unwrap();

// New, required way:
dict.set_item("key", value).unwrap();
// This works safely when `dict` is a `Bound<'py, PyDict>`
// created via `PyDict::new_bound(py)`.
```

### Type Conversions and Extracting
When extracting values back from Python strings or lists, always work within the safety of the `Bound` type system.

- Prefer `.extract::<T>()` on `Bound<'py, PyAny>` objects natively.
- Example:
```rust
let value: String = py_string.extract()?;
```

Adhering to the `Bound` API is critical to preventing compilation failures and runtime segmentation faults while integrating the Python execution capabilities in the Vox language.

---

## Automatic UV Venv Detection

`VoxPyRuntime::new()` automatically discovers the active **uv**-managed virtual environment and injects its `site-packages` directory into Python's `sys.path` **before any user imports**. This means:

- No `PYTHONPATH` environment variable is needed.
- No shell activation script (`source .venv/bin/activate`) is required.
- Packages installed via `uv sync` are importable immediately in the embedded Python.

### Detection order

The runtime checks these sources in priority order:

| # | Source | Description |
|---|---|---|
| 1 | `UV_PROJECT_ENVIRONMENT` | Environment variable set by `uv run` / `uv sync` |
| 2 | `VIRTUAL_ENV` | Standard env var when a venv is manually activated |
| 3 | `.venv/` in CWD | uv's default location after `uv sync` |
| 4 | `uv run python -c "import sysconfig; print(sysconfig.get_path('purelib'))"` | Subprocess fallback |

If none succeed, Python starts with its default `sys.path` — packages in a global interpreter will still be importable.

### Relevant functions in vox-py

```rust
/// Called automatically in VoxPyRuntime::new():
fn resolve_uv_site_packages() -> Option<PathBuf>;
fn inject_site_packages(py: Python<'_>, path: &PathBuf) -> PyResult<()>;
```

### New APIs in vox-container

`PythonEnv` now exposes:

```rust
/// Return the venv root path (not site-packages).
pub fn venv_path(&self) -> Option<PathBuf>;

/// Return the site-packages directory inside the venv.
pub fn site_packages_path(&self) -> Option<PathBuf>;
```

Use these in toolchain integrations (codegen, container init) to discover where uv has placed Python packages.

---

## VoxPyRuntime — One Constructor

There is **one way** to create a Python runtime: `VoxPyRuntime::new()`. It automatically discovers the uv-managed venv using the detection order above. This is what the Vox codegen emits — you never call it directly.

### Configuring a non-standard venv path

Set `VOX_VENV_PATH` to the venv root before running the compiled binary:

```bash
# Docker / CI — point at the absolute venv root
VOX_VENV_PATH=/app/.venv ./target/release/my-app

# Local development — do nothing; .venv is found automatically after `uv sync`
./target/release/my-app
```

`VOX_VENV_PATH` is checked as the highest-priority source inside `new()`. Windows and POSIX venv layouts (`Lib/site-packages` vs `lib/python*/site-packages`) are both handled automatically.

### Docker example

```dockerfile
FROM python:3.12-slim
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

WORKDIR /app
COPY . .
RUN uv sync

ENV VOX_VENV_PATH=/app/.venv
COPY target/release/my-app .
CMD ["./my-app"]
```

No `PYTHONPATH`. No activation script. No extra constructors.
