use burn::tensor::backend::Backend;
use burn::tensor::Distribution;

#[cfg(feature = "gpu")]
pub type VoxBackend = burn::backend::Wgpu;
#[cfg(not(feature = "gpu"))]
pub type VoxBackend = burn::backend::NdArray;

/// A dynamic-dimensional Tensor.
/// Wraps Burn's statically-dimensioned tensor to provide the dynamic usage pattern
/// expected in an interpreted or scripting-language context like Vox.
#[derive(Clone, Debug)]
pub enum Tensor<B: Backend> {
    D1(burn::tensor::Tensor<B, 1>),
    D2(burn::tensor::Tensor<B, 2>),
    D3(burn::tensor::Tensor<B, 3>),
    D4(burn::tensor::Tensor<B, 4>),
    D1Int(burn::tensor::Tensor<B, 1, burn::tensor::Int>),
    D2Int(burn::tensor::Tensor<B, 2, burn::tensor::Int>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ElementType {
    Float,
    Int,
    Bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TensorShape(pub Vec<usize>);

impl<B: Backend> Tensor<B> {
    /// Create a new 1D Tensor of zeros.
    pub fn zeros_1d(len: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 1>::zeros([len], device);
        Tensor::D1(t)
    }

    /// Create a new 2D Tensor of zeros.
    pub fn zeros_2d(rows: usize, cols: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 2>::zeros([rows, cols], device);
        Tensor::D2(t)
    }

    /// Create a new 1D Tensor of ones.
    pub fn ones_1d(len: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 1>::ones([len], device);
        Tensor::D1(t)
    }

    /// Create a new 2D Tensor of ones.
    pub fn ones_2d(rows: usize, cols: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 2>::ones([rows, cols], device);
        Tensor::D2(t)
    }

    /// Create a 1D tensor from a `Vec<f32>`.
    pub fn from_vec_1d(data: Vec<f32>, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 1>::from_floats(data.as_slice(), device);
        Tensor::D1(t)
    }

    /// Create a 3D tensor from a 1D array.
    pub fn from_vec_3d(data: Vec<f32>, d0: usize, d1: usize, d2: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 1>::from_floats(data.as_slice(), device)
            .reshape([d0, d1, d2]);
        Tensor::D3(t)
    }

    /// Create a 2D tensor from a flat `Vec<f32>` with explicit shape [rows, cols].
    pub fn from_vec_2d(data: Vec<f32>, rows: usize, cols: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 1>::from_floats(data.as_slice(), device)
            .reshape([rows, cols]);
        Tensor::D2(t)
    }

    /// Create a 2D tensor with random normal values (mean=0, std=1).
    pub fn randn_2d(rows: usize, cols: usize, device: &B::Device) -> Self {
        let t = burn::tensor::Tensor::<B, 2>::random([rows, cols], Distribution::Normal(0.0, 1.0), device);
        Tensor::D2(t)
    }

    /// Retrieve the shape of the tensor.
    pub fn shape(&self) -> TensorShape {
        match self {
            Tensor::D1(t) => TensorShape(t.shape().dims.into()),
            Tensor::D2(t) => TensorShape(t.shape().dims.into()),
            Tensor::D3(t) => TensorShape(t.shape().dims.into()),
            Tensor::D4(t) => TensorShape(t.shape().dims.into()),
            Tensor::D1Int(t) => TensorShape(t.shape().dims.into()),
            Tensor::D2Int(t) => TensorShape(t.shape().dims.into()),
        }
    }

    /// Perform a matrix multiplication. Only valid for 2D tensors.
    pub fn matmul(&self, other: &Tensor<B>) -> Self {
        match (self, other) {
            (Tensor::D2(a), Tensor::D2(b)) => Tensor::D2(a.clone().matmul(b.clone())),
            _ => panic!("matmul currently only supports 2D Float tensors in vox-tensor"),
        }
    }

    /// Element-wise addition.
    pub fn add(&self, other: &Tensor<B>) -> Self {
        match (self, other) {
            (Tensor::D1(a), Tensor::D1(b)) => Tensor::D1(a.clone() + b.clone()),
            (Tensor::D2(a), Tensor::D2(b)) => Tensor::D2(a.clone() + b.clone()),
            (Tensor::D1Int(a), Tensor::D1Int(b)) => Tensor::D1Int(a.clone() + b.clone()),
            (Tensor::D2Int(a), Tensor::D2Int(b)) => Tensor::D2Int(a.clone() + b.clone()),
            _ => panic!("add: dimension or type mismatch"),
        }
    }

    /// Element-wise subtraction.
    pub fn sub(&self, other: &Tensor<B>) -> Self {
        match (self, other) {
            (Tensor::D1(a), Tensor::D1(b)) => Tensor::D1(a.clone() - b.clone()),
            (Tensor::D2(a), Tensor::D2(b)) => Tensor::D2(a.clone() - b.clone()),
            (Tensor::D1Int(a), Tensor::D1Int(b)) => Tensor::D1Int(a.clone() - b.clone()),
            (Tensor::D2Int(a), Tensor::D2Int(b)) => Tensor::D2Int(a.clone() - b.clone()),
            _ => panic!("sub: dimension or type mismatch"),
        }
    }

    /// Element-wise multiplication.
    pub fn mul(&self, other: &Tensor<B>) -> Self {
        match (self, other) {
            (Tensor::D1(a), Tensor::D1(b)) => Tensor::D1(a.clone() * b.clone()),
            (Tensor::D2(a), Tensor::D2(b)) => Tensor::D2(a.clone() * b.clone()),
            (Tensor::D1Int(a), Tensor::D1Int(b)) => Tensor::D1Int(a.clone() * b.clone()),
            (Tensor::D2Int(a), Tensor::D2Int(b)) => Tensor::D2Int(a.clone() * b.clone()),
            _ => panic!("mul: dimension or type mismatch"),
        }
    }

    /// Transpose a 2D tensor.
    pub fn transpose(&self) -> Self {
        match self {
            Tensor::D2(t) => Tensor::D2(t.clone().transpose()),
            Tensor::D2Int(t) => Tensor::D2Int(t.clone().transpose()),
            _ => panic!("transpose is only valid for 2D tensors"),
        }
    }

    /// Apply element-wise ReLU activation.
    pub fn relu(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::relu(t.clone())),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::relu(t.clone())),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::relu(t.clone())),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::relu(t.clone())),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("relu: unsupported for int tensors"),
        }
    }

    /// Apply element-wise Tanh activation.
    pub fn tanh(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(t.clone().tanh()),
            Tensor::D2(t) => Tensor::D2(t.clone().tanh()),
            Tensor::D3(t) => Tensor::D3(t.clone().tanh()),
            Tensor::D4(t) => Tensor::D4(t.clone().tanh()),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("tanh: unsupported for int tensors"),
        }
    }

    /// Apply element-wise Sigmoid activation.
    pub fn sigmoid(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::sigmoid(t.clone())),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::sigmoid(t.clone())),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::sigmoid(t.clone())),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::sigmoid(t.clone())),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("sigmoid: unsupported for int tensors"),
        }
    }

    /// Apply element-wise GELU activation.
    pub fn gelu(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::gelu(t.clone())),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::gelu(t.clone())),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::gelu(t.clone())),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::gelu(t.clone())),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("gelu: unsupported for int tensors"),
        }
    }

    /// Apply element-wise SiLU (Swish) activation.
    pub fn silu(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::silu(t.clone())),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::silu(t.clone())),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::silu(t.clone())),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::silu(t.clone())),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("silu: unsupported for Int tensors"),
        }
    }

    /// Apply element-wise Leaky ReLU activation.
    pub fn leaky_relu(&self, negative_slope: f64) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::leaky_relu(t.clone(), negative_slope)),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::leaky_relu(t.clone(), negative_slope)),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::leaky_relu(t.clone(), negative_slope)),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::leaky_relu(t.clone(), negative_slope)),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("leaky_relu: unsupported for Int tensors"),
        }
    }

    /// Apply Softmax activation along a dimension.
    pub fn softmax(&self, dim: usize) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(burn::tensor::activation::softmax(t.clone(), dim)),
            Tensor::D2(t) => Tensor::D2(burn::tensor::activation::softmax(t.clone(), dim)),
            Tensor::D3(t) => Tensor::D3(burn::tensor::activation::softmax(t.clone(), dim)),
            Tensor::D4(t) => Tensor::D4(burn::tensor::activation::softmax(t.clone(), dim)),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("softmax: unsupported for Int tensors"),
        }
    }

    /// Concatenate multiple tensors along a dimension.
    pub fn cat(tensors: Vec<Self>, dim: usize) -> Self {
        if tensors.is_empty() {
            panic!("cat: empty tensor list");
        }
        match &tensors[0] {
            Tensor::D1(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 1>> = tensors.into_iter().map(|t| match t {
                    Tensor::D1(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D1(burn::tensor::Tensor::cat(inner, dim))
            }
            Tensor::D2(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 2>> = tensors.into_iter().map(|t| match t {
                    Tensor::D2(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D2(burn::tensor::Tensor::cat(inner, dim))
            }
            Tensor::D3(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 3>> = tensors.into_iter().map(|t| match t {
                    Tensor::D3(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D3(burn::tensor::Tensor::cat(inner, dim))
            }
            Tensor::D4(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 4>> = tensors.into_iter().map(|t| match t {
                    Tensor::D4(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D4(burn::tensor::Tensor::cat(inner, dim))
            }
            Tensor::D1Int(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 1, burn::tensor::Int>> = tensors.into_iter().map(|t| match t {
                    Tensor::D1Int(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D1Int(burn::tensor::Tensor::cat(inner, dim))
            }
            Tensor::D2Int(_) => {
                let inner: Vec<burn::tensor::Tensor<B, 2, burn::tensor::Int>> = tensors.into_iter().map(|t| match t {
                    Tensor::D2Int(it) => it,
                    _ => panic!("cat: rank mismatch"),
                }).collect();
                Tensor::D2Int(burn::tensor::Tensor::cat(inner, dim))
            }
        }
    }

    /// Reshape the tensor to a new shape.
    pub fn reshape(&self, shape: Vec<usize>) -> Self {
        let n_dims = shape.len();
        match self {
            Tensor::D1(t) => match n_dims {
                1 => Tensor::D1(t.clone().reshape([shape[0]])),
                2 => Tensor::D2(t.clone().reshape([shape[0], shape[1]])),
                3 => Tensor::D3(t.clone().reshape([shape[0], shape[1], shape[2]])),
                4 => Tensor::D4(t.clone().reshape([shape[0], shape[1], shape[2], shape[3]])),
                _ => panic!("reshape: unsupported rank"),
            },
            Tensor::D2(t) => match n_dims {
                1 => Tensor::D1(t.clone().reshape([shape[0]])),
                2 => Tensor::D2(t.clone().reshape([shape[0], shape[1]])),
                3 => Tensor::D3(t.clone().reshape([shape[0], shape[1], shape[2]])),
                4 => Tensor::D4(t.clone().reshape([shape[0], shape[1], shape[2], shape[3]])),
                _ => panic!("reshape: unsupported rank"),
            },
            Tensor::D3(t) => match n_dims {
                1 => Tensor::D1(t.clone().reshape([shape[0]])),
                2 => Tensor::D2(t.clone().reshape([shape[0], shape[1]])),
                3 => Tensor::D3(t.clone().reshape([shape[0], shape[1], shape[2]])),
                4 => Tensor::D4(t.clone().reshape([shape[0], shape[1], shape[2], shape[3]])),
                _ => panic!("reshape: unsupported rank"),
            },
            Tensor::D4(t) => match n_dims {
                1 => Tensor::D1(t.clone().reshape([shape[0]])),
                2 => Tensor::D2(t.clone().reshape([shape[0], shape[1]])),
                3 => Tensor::D3(t.clone().reshape([shape[0], shape[1], shape[2]])),
                4 => Tensor::D4(t.clone().reshape([shape[0], shape[1], shape[2], shape[3]])),
                _ => panic!("reshape: unsupported rank"),
            },
            Tensor::D1Int(t) => match n_dims {
                1 => Tensor::D1Int(t.clone().reshape([shape[0]])),
                2 => Tensor::D2Int(t.clone().reshape([shape[0], shape[1]])),
                _ => panic!("reshape: unsupported rank for Int"),
            },
            Tensor::D2Int(t) => match n_dims {
                1 => Tensor::D1Int(t.clone().reshape([shape[0]])),
                2 => Tensor::D2Int(t.clone().reshape([shape[0], shape[1]])),
                _ => panic!("reshape: unsupported rank for Int"),
            },
        }
    }

    /// Generate a 1D tensor with values from `start` to `end` (exclusive) with `step`.
    pub fn arange(start: f32, end: f32, step: f32, device: &B::Device) -> Self {
        let count = ((end - start) / step).ceil() as usize;
        let mut data = Vec::with_capacity(count);
        let mut val = start;
        for _ in 0..count {
            data.push(val);
            val += step;
        }
        Tensor::D1(burn::tensor::Tensor::<B, 1>::from_floats(data.as_slice(), device))
    }

    /// Generate a 1D tensor with `steps` values from `start` to `end` (inclusive).
    pub fn linspace(start: f32, end: f32, steps: usize, device: &B::Device) -> Self {
        if steps == 0 {
            return Tensor::D1(burn::tensor::Tensor::<B, 1>::from_floats([0.0; 0], device));
        }
        if steps == 1 {
            return Tensor::D1(burn::tensor::Tensor::<B, 1>::from_floats([start], device));
        }
        let step = (end - start) / (steps - 1) as f32;
        let mut data = Vec::with_capacity(steps);
        for i in 0..steps {
            data.push(start + i as f32 * step);
        }
        Tensor::D1(burn::tensor::Tensor::<B, 1>::from_floats(data.as_slice(), device))
    }

    /// Add a dimension of size 1 at the specified position.
    pub fn unsqueeze(&self, dim: usize) -> Self {
        match self {
            Tensor::D1(t) => match dim {
                0 => Tensor::D2(t.clone().unsqueeze_dim(0)),
                1 => Tensor::D2(t.clone().unsqueeze_dim(1)),
                _ => panic!("unsqueeze: invalid dim for D1"),
            },
            Tensor::D2(t) => match dim {
                0 => Tensor::D3(t.clone().unsqueeze_dim(0)),
                1 => Tensor::D3(t.clone().unsqueeze_dim(1)),
                2 => Tensor::D3(t.clone().unsqueeze_dim(2)),
                _ => panic!("unsqueeze: invalid dim for D2"),
            },
            Tensor::D3(t) => match dim {
                0 => Tensor::D4(t.clone().unsqueeze_dim(0)),
                1 => Tensor::D4(t.clone().unsqueeze_dim(1)),
                2 => Tensor::D4(t.clone().unsqueeze_dim(2)),
                3 => Tensor::D4(t.clone().unsqueeze_dim(3)),
                _ => panic!("unsqueeze: invalid dim for D3"),
            },
            Tensor::D4(_) => panic!("unsqueeze: unsupported rank or already at max rank"),
            Tensor::D1Int(t) => match dim {
                0 => Tensor::D2Int(t.clone().unsqueeze_dim(0)),
                1 => Tensor::D2Int(t.clone().unsqueeze_dim(1)),
                _ => panic!("unsqueeze: invalid dim for D1Int"),
            },
            Tensor::D2Int(_) => panic!("unsqueeze: unsupported rank or already at max rank"),
        }
    }

    /// Stack tensors along a new dimension.
    pub fn stack(tensors: Vec<Self>, dim: usize) -> Self {
        let unsqueezed: Vec<Self> = tensors.into_iter().map(|t| t.unsqueeze(dim)).collect();
        Self::cat(unsqueezed, dim)
    }

    /// Slice a tensor along a dimension.
    pub fn slice(&self, dim: usize, start: usize, end: usize) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(t.clone().slice([start..end])),
            Tensor::D2(t) => match dim {
                0 => Tensor::D2(t.clone().slice([start..end, 0..t.dims()[1]])),
                1 => Tensor::D2(t.clone().slice([0..t.dims()[0], start..end])),
                _ => panic!("slice: invalid dim for D2"),
            },
            Tensor::D3(t) => match dim {
                0 => Tensor::D3(t.clone().slice([start..end, 0..t.dims()[1], 0..t.dims()[2]])),
                1 => Tensor::D3(t.clone().slice([0..t.dims()[0], start..end, 0..t.dims()[2]])),
                2 => Tensor::D3(t.clone().slice([0..t.dims()[0], 0..t.dims()[1], start..end])),
                _ => panic!("slice: invalid dim for D3"),
            },
            Tensor::D4(t) => match dim {
                0 => Tensor::D4(t.clone().slice([start..end, 0..t.dims()[1], 0..t.dims()[2], 0..t.dims()[3]])),
                1 => Tensor::D4(t.clone().slice([0..t.dims()[0], start..end, 0..t.dims()[2], 0..t.dims()[3]])),
                2 => Tensor::D4(t.clone().slice([0..t.dims()[0], 0..t.dims()[1], start..end, 0..t.dims()[3]])),
                3 => Tensor::D4(t.clone().slice([0..t.dims()[0], 0..t.dims()[1], 0..t.dims()[2], start..end])),
                _ => panic!("slice: invalid dim for D4"),
            },
            Tensor::D1Int(t) => Tensor::D1Int(t.clone().slice([start..end])),
            Tensor::D2Int(t) => match dim {
                0 => Tensor::D2Int(t.clone().slice([start..end, 0..t.dims()[1]])),
                1 => Tensor::D2Int(t.clone().slice([0..t.dims()[0], start..end])),
                _ => panic!("slice: invalid dim for D2Int"),
            },
        }
    }

    /// Sum all elements into a scalar tensor (1D, size 1).
    pub fn sum(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(t.clone().sum().unsqueeze()),
            Tensor::D2(t) => Tensor::D1(t.clone().sum().unsqueeze()),
            Tensor::D3(t) => Tensor::D1(t.clone().sum().unsqueeze()),
            Tensor::D4(t) => Tensor::D1(t.clone().sum().unsqueeze()),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("sum: unsupported for int tensors"),
        }
    }

    /// Compute mean of all elements.
    pub fn mean(&self) -> Self {
        match self {
            Tensor::D1(t) => Tensor::D1(t.clone().mean().unsqueeze()),
            Tensor::D2(t) => Tensor::D1(t.clone().mean().unsqueeze()),
            Tensor::D3(t) => Tensor::D1(t.clone().mean().unsqueeze()),
            Tensor::D4(t) => Tensor::D1(t.clone().mean().unsqueeze()),
            Tensor::D1Int(_) | Tensor::D2Int(_) => panic!("mean: unsupported for int tensors"),
        }
    }

    /// Extract all values as a flat Vec<f32>.
    pub fn to_vec(&self) -> Vec<f32> {
        match self {
            Tensor::D1(t) => t.clone().into_data().to_vec::<f32>().unwrap_or_default(),
            Tensor::D2(t) => t.clone().into_data().to_vec::<f32>().unwrap_or_default(),
            Tensor::D3(t) => t.clone().into_data().to_vec::<f32>().unwrap_or_default(),
            Tensor::D4(t) => t.clone().into_data().to_vec::<f32>().unwrap_or_default(),
            Tensor::D1Int(t) => t.clone().into_data().to_vec::<i32>().unwrap_or_default().into_iter().map(|v| v as f32).collect(),
            Tensor::D2Int(t) => t.clone().into_data().to_vec::<i32>().unwrap_or_default().into_iter().map(|v| v as f32).collect(),
        }
    }

    /// Number of elements.
    pub fn numel(&self) -> usize {
        self.shape().0.iter().product()
    }
}
