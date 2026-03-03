use burn::backend::NdArray;
use burn::tensor::Tensor;
use vox_tensor::nn::Module;

type B = NdArray<f32>;

#[test]
fn test_linear_forward() {
    let linear_module = Module::<B>::linear(10, 5, true);

    // Create an input tensor (e.g., batch_size: 2, features: 10)
    let device = Default::default();
    let input = Tensor::<B, 2>::ones([2, 10], &device);

    if let Module::Linear(linear) = linear_module {
        let output = linear.forward(input);

        let shape = output.shape().dims;
        assert_eq!(shape, [2, 5], "Output shape should be [2, 5]");
    } else {
        panic!("Expected Module::Linear");
    }
}

#[test]
fn test_dropout_forward() {
    let dropout_module = Module::<B>::dropout(0.5);

    let device = Default::default();
    let input = Tensor::<B, 2>::ones([2, 10], &device);

    if let Module::Dropout(dropout) = dropout_module {
        let output = dropout.forward(input);

        let shape = output.shape().dims;
        assert_eq!(shape, [2, 10], "Output shape should be [2, 10]");
    } else {
        panic!("Expected Module::Dropout");
    }
}

#[test]
fn test_batch_norm_forward() {
    let bn_module = Module::<B>::batch_norm(10);

    let device = Default::default();
    let input = Tensor::<B, 2>::ones([2, 10], &device);

    if let Module::BatchNorm(bn) = bn_module {
        let output = bn.forward(input);

        let shape = output.shape().dims;
        assert_eq!(shape, [2, 10], "Output shape should be [2, 10]");
    } else {
        panic!("Expected Module::BatchNorm");
    }
}

#[test]
fn test_layer_norm_forward() {
    let ln_module = Module::<B>::layer_norm(10);

    let device = Default::default();
    let input = Tensor::<B, 2>::ones([2, 10], &device);

    if let Module::LayerNorm(ln) = ln_module {
        let output = ln.forward(input);

        let shape = output.shape().dims;
        assert_eq!(shape, [2, 10], "Output shape should be [2, 10]");
    } else {
        panic!("Expected Module::LayerNorm");
    }
}

#[test]
fn test_embedding_forward() {
    let em_module = Module::<B>::embedding(100, 10);

    let device = Default::default();

    // Create an integer input tensor (Burn 0.19 requires Int for Embedding inputs)
    let input = burn::tensor::Tensor::<B, 2, burn::tensor::Int>::from_data(
        [[0i64, 1i64, 2i64], [3i64, 4i64, 5i64]],
        &device
    );

    if let Module::Embedding(em) = em_module {
        let output = em.forward(input);

        let shape = output.shape().dims;
        assert_eq!(shape, [2, 3, 10], "Output shape should be [2, 3, 10]");
    } else {
        panic!("Expected Module::Embedding");
    }
}
