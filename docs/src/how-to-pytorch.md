# @py.import – Python Library Integration (`torch`, `numpy`, etc.)

Vox can import and call Python libraries directly from `.vox` files using the `@py.import` decorator. The Python interpreter is embedded at runtime via `pyo3`, and packages are managed by **[uv](https://docs.astral.sh/uv/)** — the fast Python package manager built in Rust. No manual `PYTHONPATH` configuration is required.
## Quick Start

```vox
@py.import torch
@py.import torch.nn as nn

fn run_inference(input: list[float]) to list[float]:
    let t = torch.tensor(input)
    let model = nn.Linear(4, 1)
    ret model.forward(t).tolist()
```

Run once to set up the environment:

```bash
vox container init --file src/main.vox
```

That's it. The command auto-installs Python 3.12, creates a `.venv` directory, and installs required packages. Your compiled binary will find the packages automatically at runtime.

## Syntax

```
@py.import <module>                   # binds to last segment (torch → torch)
@py.import <module> as <alias>        # custom binding (torch.nn → nn)
```

Both dotted module paths (`torch.nn.functional`) and simple names (`torch`) are supported.

## How It Works

`vox container init` runs the full setup flow using **uv**:

1. Detects your environment (uv, Python version, GPU/CUDA).
2. Runs `uv python install 3.12` — idempotent, skips if already installed.
3. Generates a `pyproject.toml` with the correct PyTorch wheel source (CPU or CUDA).
4. Runs `uv sync` — creates `.venv` in your project directory.

At runtime, the `vox-py` bridge auto-detects the `.venv` and injects its `site-packages` into Python's `sys.path`. **No `PYTHONPATH` or shell activation is needed.**

### venv discovery order

The runtime looks for the venv in this order:

| Priority | Source |
|---|---|
| 1 | `UV_PROJECT_ENVIRONMENT` env var (set by `uv run`) |
| 2 | `VIRTUAL_ENV` env var (manual activation) |
| 3 | `.venv` in the current working directory |
| 4 | Subprocess query: `uv run python -c "import sys; print(sys.prefix)"` |

### Type conversions

Inputs are automatically converted from Vox types to Python types:

| Vox type      | Python type    |
|---------------|----------------|
| `int`         | `int`          |
| `float`       | `float`        |
| `str`         | `str`          |
| `bool`        | `bool`         |
| `list[T]`     | `list`         |
| `dict`        | `dict`         |

Return values come back as their string representation. Use helper utilities like `PY_RT.tensor_to_vec_f64()` to convert tensors to Vox-native lists, or `PY_RT.to_json()` for structured results.

## PyTorch Example

```vox
@py.import torch
@py.import torch.nn as nn
@py.import torch.nn.functional as F

# Build and run a 2-layer MLP
fn mlp_forward(x: list[float]) to list[float]:
    let t       = torch.tensor(x)
    let linear1 = nn.Linear(4, 8)
    let linear2 = nn.Linear(8, 2)
    let h       = F.relu(linear1.forward(t))
    let out     = linear2.forward(h)
    ret out.tolist()

fn main():
    let result = mlp_forward([1.0, 2.0, 3.0, 4.0])
    print(result)
```

## NumPy Example

```vox
@py.import numpy as np

fn moving_average(data: list[float], window: int) to list[float]:
    let arr = np.array(data)
    let weights = np.ones(window) / window
    ret np.convolve(arr, weights, "valid").tolist()
```

## Runtime Environment

`vox container init` handles everything:

```bash
# First time — installs Python 3.12, creates .venv, installs packages
vox container init --file src/main.vox

# Subsequent runs — just rebuild and run; .venv is already there
cargo build && ./target/debug/my-app
```

### Docker / CI

When the venv lives at a non-standard path (e.g. inside a Docker image), set `VOX_VENV_PATH` to override auto-detection:

```dockerfile
FROM python:3.12-slim
RUN pip install uv

WORKDIR /app
COPY . .
RUN uv sync

# VOX_VENV_PATH tells the compiled binary exactly where packages live
ENV VOX_VENV_PATH=/app/.venv
CMD ["./target/release/my-app"]
```

Or in a CI step:

```yaml
- run: |
    uv sync
    cargo build --release
    VOX_VENV_PATH=$(pwd)/.venv ./target/release/my-app
```

> [!TIP]
> For GPU workloads, run on a machine with an NVIDIA GPU before calling `vox container init`. The toolchain auto-detects CUDA and selects the correct PyTorch wheels.

> [!NOTE]
> The `vox-py` Cargo feature is disabled by default to keep compile times short. Enable it by adding `vox-py` as a dependency to your project's `Cargo.toml`.

> [!IMPORTANT]
> **Do not set `PYTHONPATH` manually.** The `vox-py` runtime discovers the uv-managed `.venv` automatically. Setting `PYTHONPATH` to a different environment will override this detection and may cause import errors.

## CUDA Configuration

Vox auto-selects the right PyTorch wheel source based on your detected GPU:

| Detected CUDA | PyTorch index |
|---|---|
| 13.x | `cu130` |
| 12.4–12.6 | `cu124` |
| 12.1–12.3 | `cu121` |
| 11.8 | `cu118` |
| None / CPU | `cpu` |

## Available Bridge Methods

| Method | Description |
|--------|-------------|
| `PY_RT.call_method(alias, method, args)` | Positional args |
| `PY_RT.call_method_kwargs(alias, method, args, kwargs)` | Positional + keyword args |
| `PY_RT.call_method0(alias, method)` | Zero-arg call |
| `PY_RT.get_attr(alias, attr_path)` | Get attribute value as string |
| `PY_RT.tensor_to_vec_f64(alias, repr)` | Extract tensor → `Vec<f64>` |
| `PY_RT.to_json(alias, expr)` | Extract any Python value → JSON |
| `PY_RT.eval(alias, expression)` | Evaluate arbitrary Python expression |


## See Also

- [`@py.import` decorator reference](ref-decorators.md#pyimport)
- [PyTorch inference example](../examples/pytorch_inference.vox)
- [NumPy integration patterns](how-to-ai-agents.md)

---

## The Future: Native Vox ML (`vox-tensor`)

While Python integration provides tremendous utility today, it inherently violates deeply-held Vox principles: **Zero dependency drift**, **One Binary deployment**, and **Complete cross-platform compilation**.

To address this, we have implemented **`vox-tensor`** — a native ML layer built on the [Burn](https://burn.dev) framework, providing 95% of PyTorch's capabilities without Python.

### Current API (implemented)

```rust
// Tensor creation
Tensor::zeros_1d(len)               // 1D zero tensor
Tensor::zeros_2d(rows, cols)        // 2D zero tensor
Tensor::ones_1d(len)                // 1D ones tensor
Tensor::ones_2d(rows, cols)         // 2D ones tensor
Tensor::from_vec_1d(data)           // 1D from Vec<f32>
Tensor::from_vec_2d(data, rows, cols) // 2D from Vec<f32>
Tensor::randn_1d(len)               // 1D random normal
Tensor::randn_2d(rows, cols)        // 2D random normal

// Operations
tensor.add(&other)           // element-wise add
tensor.sub(&other)           // element-wise subtract
tensor.mul(&other)           // element-wise multiply
tensor.mul_scalar(f64)       // scalar multiply
tensor.add_scalar(f64)       // scalar add
tensor.matmul(&other)        // matrix multiply (2D only)
tensor.transpose()           // transpose (2D only)
tensor.relu()                // ReLU activation
tensor.sigmoid()             // sigmoid activation
tensor.sum()                 // sum all elements
tensor.mean()                // mean all elements
tensor.to_vec()              // extract to Vec<f32>
tensor.shape()               // TensorShape
tensor.numel()               // total element count
```

### Neural Network Layers

```rust
// Layers
nn::Module::linear(in, out, bias)   // Dense layer
nn::Module::dropout(prob)           // Dropout
nn::Module::batch_norm1d(features)  // BatchNorm1d
nn::Module::conv2d(in_ch, out_ch, kernel) // Conv2d

// Composition
nn::Sequential::new(vec![
    Module::linear(4, 8, true),
    Module::linear(8, 2, true),
])
.forward(input_tensor)
```

### Example: MLP inference without Python

```vox
import tensor as t
import nn

let model = nn.Sequential([
    nn.Module::linear(4, 8, true),
    nn.Module::linear(8, 2, true),
])

let input = t.Tensor::from_vec_2d([1.0, 2.0, 3.0, 4.0], 1, 4)
let out = model.forward(input)
ret out.to_vec()
```

This ensures **Low K-Complexity** (no shell dependencies), native type-checked operations, and deployment via the built-in HTTP server — all in a single, self-contained binary.

> [!NOTE]
> `vox-tensor` uses **NdArray** (CPU) as the default backend with **Autodiff** for gradient tracking.
> GPU acceleration (WGPU) is available via the `wgpu` feature flag in `vox-tensor/Cargo.toml`.
