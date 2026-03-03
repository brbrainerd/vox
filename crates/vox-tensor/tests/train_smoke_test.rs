use burn::backend::{Autodiff, NdArray};
use burn::tensor::{Distribution, Tensor};
use burn::optim::{AdamWConfig, Optimizer};
use vox_tensor::nn::Module;

// The backend to use for the smoke test (CPU-only, no Autodiff)
type B = NdArray<f32>;
// For training, we temporarily use Autodiff over NdArray.
type AB = Autodiff<B>;

fn train_step_smoke() {
    let device = Default::default();

    // 1. Initialize a simple model (a single linear layer)
    let linear_module = Module::<AB>::linear(10, 5, true);
    let mut linear = match linear_module {
        Module::Linear(l) => l,
        _ => panic!("Expected Module::Linear"),
    };

    // 2. Initialize the optimizer (AdamW)
    let adamw_config = AdamWConfig::new();
    let mut optim = adamw_config.init::<AB, burn::nn::Linear<AB>>();

    let learning_rate = 1e-3;

    // 3. Create dummy input and target pairs
    let input = Tensor::<AB, 2>::random([2, 10], Distribution::Normal(0.0, 1.0), &device);
    let target = Tensor::<AB, 2>::ones([2, 5], &device);

    for _epoch in 0..5 {
        // Forward pass
        let output = linear.forward(input.clone());

        // Calculate loss (MSE)
        let loss = output.sub(target.clone()).powf_scalar(2.0).mean();

        // Backward pass
        let grads = loss.backward();

        // Optimizer step
        let grads_params = burn::optim::GradientsParams::from_grads(grads, &linear);
        linear = optim.step(learning_rate, linear, grads_params);
    }

    // Just verify the shape of the trained parameters remains the same
    let final_weight_shape = linear.weight.val().shape().dims;
    assert_eq!(final_weight_shape, [10, 5]);
}

#[test]
fn test_native_training_loop_cpu_smoke() {
    train_step_smoke();
}
