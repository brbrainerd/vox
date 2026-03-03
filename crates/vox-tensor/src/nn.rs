use crate::tensor::Tensor;
use burn::module::{Module as BurnModule, Param};
use burn::tensor::backend::Backend;

/// Internal struct for LoRA parameters to enable Burn's parameter tracking.
#[derive(BurnModule, Debug)]
pub struct LoraParams<B: Backend> {
    pub a: Param<burn::tensor::Tensor<B, 2>>,
    pub b: Param<burn::tensor::Tensor<B, 2>>,
}

/// LoRA adapter: frozen base Linear + two small trainable matrices.
#[derive(BurnModule, Debug)]
pub struct LoraLinear<B: Backend> {
    pub base: burn::nn::Linear<B>,
    pub lora: LoraParams<B>,
    pub scaling: f64,
}

impl<B: Backend> LoraLinear<B> {
    pub fn forward(&self, x: burn::tensor::Tensor<B, 2>) -> burn::tensor::Tensor<B, 2> {
        let base_out = self.base.forward(x.clone());
        let a_t = self.lora.a.val().transpose();
        let b_t = self.lora.b.val().transpose();
        let lora_out = x.matmul(a_t).matmul(b_t);
        base_out + lora_out * (self.scaling as f32)
    }
}

/// Dynamic wrapping of `burn::nn` layers for Vox interpretation.
/// Mirrors the PyTorch `nn.Module` interface.
#[derive(BurnModule, Debug)]
pub enum Module<B: Backend> {
    Linear(burn::nn::Linear<B>),
    Dropout(burn::nn::Dropout),
    BatchNorm(burn::nn::BatchNorm<B>),
    Conv1d(burn::nn::conv::Conv1d<B>),
    Conv2d(burn::nn::conv::Conv2d<B>),
    // LSTM: burn::nn::lstm module (Burn ≥0.14). Falls back to disabled if module unavailable.
    #[cfg(feature = "lstm")]
    Lstm(burn::nn::lstm::Lstm<B>),
    LoraLinear(LoraLinear<B>),
    Embedding(burn::nn::Embedding<B>),
    LayerNorm(burn::nn::LayerNorm<B>),
    MultiHeadAttention(burn::nn::attention::MultiHeadAttention<B>),
    TransformerEncoder(burn::nn::transformer::TransformerEncoder<B>),
}

impl<B: Backend> Module<B> {
    pub fn linear(in_features: usize, out_features: usize, bias: bool) -> Self {
        let device = Default::default();
        let config = burn::nn::LinearConfig::new(in_features, out_features).with_bias(bias);
        Module::Linear(config.init(&device))
    }

    pub fn dropout(prob: f64) -> Self {
        let config = burn::nn::DropoutConfig::new(prob);
        Module::Dropout(config.init())
    }

    pub fn batch_norm(num_features: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::BatchNormConfig::new(num_features);
        Module::BatchNorm(config.init(&device))
    }

    pub fn conv1d(channels_in: usize, channels_out: usize, kernel_size: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::conv::Conv1dConfig::new(
            channels_in, channels_out, kernel_size,
        );
        Module::Conv1d(config.init(&device))
    }

    pub fn conv2d(channels_in: usize, channels_out: usize, kernel_size: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::conv::Conv2dConfig::new(
            [channels_in, channels_out],
            [kernel_size, kernel_size],
        );
        Module::Conv2d(config.init(&device))
    }

    #[cfg(feature = "lstm")]
    pub fn lstm(d_input: usize, d_hidden: usize, bias: bool) -> Self {
        let device = Default::default();
        let config = burn::nn::lstm::LstmConfig::new(d_input, d_hidden, bias);
        Module::Lstm(config.init(&device))
    }

    pub fn lora_linear(in_features: usize, out_features: usize, rank: usize, alpha: f64) -> Self {
        let device = Default::default();
        let base_config = burn::nn::LinearConfig::new(in_features, out_features).with_bias(true);
        let base = base_config.init(&device);
        let a = burn::tensor::Tensor::<B, 2>::random(
            [rank, in_features],
            burn::tensor::Distribution::Normal(0.0, 0.02),
            &device,
        );
        let b = burn::tensor::Tensor::<B, 2>::zeros([out_features, rank], &device);
        let scaling = alpha / rank as f64;
        Module::LoraLinear(LoraLinear {
            base,
            lora: LoraParams {
                a: Param::from_tensor(a),
                b: Param::from_tensor(b)
            },
            scaling
        })
    }

    pub fn embedding(num_embeddings: usize, embedding_dim: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::EmbeddingConfig::new(num_embeddings, embedding_dim);
        Module::Embedding(config.init(&device))
    }

    pub fn layer_norm(d_model: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::LayerNormConfig::new(d_model);
        Module::LayerNorm(config.init(&device))
    }

    pub fn multi_head_attention(d_model: usize, num_heads: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::attention::MultiHeadAttentionConfig::new(d_model, num_heads);
        Module::MultiHeadAttention(config.init(&device))
    }

    pub fn transformer_encoder(d_model: usize, d_ff: usize, n_heads: usize, n_layers: usize) -> Self {
        let device = Default::default();
        let config = burn::nn::transformer::TransformerEncoderConfig::new(d_model, d_ff, n_heads, n_layers);
        Module::TransformerEncoder(config.init(&device))
    }

    pub fn forward(&self, x: &Tensor<B>) -> Tensor<B> {
        match self {
            Module::Linear(layer) => match x {
                Tensor::D2(input) => Tensor::D2(layer.forward(input.clone())),
                _ => panic!("Linear expects D2"),
            },
            Module::Dropout(layer) => match x {
                Tensor::D1(input) => Tensor::D1(layer.forward(input.clone())),
                Tensor::D2(input) => Tensor::D2(layer.forward(input.clone())),
                Tensor::D3(input) => Tensor::D3(layer.forward(input.clone())),
                _ => panic!("Dropout: unsupported rank"),
            },
            Module::BatchNorm(layer) => match x {
                Tensor::D2(input) => Tensor::D2(layer.forward(input.clone())),
                Tensor::D3(input) => Tensor::D3(layer.forward(input.clone())),
                _ => panic!("BatchNorm expects rank >= 2"),
            },
            Module::Conv1d(layer) => match x {
                Tensor::D3(input) => Tensor::D3(layer.forward(input.clone())),
                _ => panic!("Conv1d expects D3"),
            },
            Module::Conv2d(layer) => match x {
                Tensor::D4(input) => Tensor::D4(layer.forward(input.clone())),
                _ => panic!("Conv2d expects D4"),
            },
            #[cfg(feature = "lstm")]
            Module::Lstm(layer) => match x {
                Tensor::D3(input) => {
                    let (output, _state) = layer.forward(input.clone(), None);
                    Tensor::D3(output)
                }
                _ => panic!("LSTM expects D3 [batch, seq_len, d_input]"),
            },
            Module::LoraLinear(layer) => match x {
                Tensor::D2(input) => Tensor::D2(layer.forward(input.clone())),
                _ => panic!("LoraLinear expects D2"),
            },
            Module::Embedding(layer) => match x {
                Tensor::D2(input) => Tensor::D3(layer.forward(input.clone().int())),
                _ => panic!("Embedding expects D2 (int)"),
            },
            Module::LayerNorm(layer) => match x {
                Tensor::D2(input) => Tensor::D2(layer.forward(input.clone())),
                Tensor::D3(input) => Tensor::D3(layer.forward(input.clone())),
                _ => panic!("LayerNorm expects D2 or D3"),
            },
            Module::MultiHeadAttention(layer) => match x {
                Tensor::D3(input) => {
                    let mha_input = burn::nn::attention::MhaInput::self_attn(input.clone());
                    let mha_output = layer.forward(mha_input);
                    Tensor::D3(mha_output.context)
                }
                _ => panic!("MHA expects D3"),
            },
            Module::TransformerEncoder(layer) => match x {
                Tensor::D3(input) => {
                    let enc_input = burn::nn::transformer::TransformerEncoderInput::new(input.clone());
                    Tensor::D3(layer.forward(enc_input))
                }
                _ => panic!("TransformerEncoder expects D3 [batch, seq_len, d_model]"),
            },
        }
    }
}

/// A Sequential container — executes modules in order.
#[derive(BurnModule, Debug)]
pub struct Sequential<B: Backend> {
    layers: Vec<Module<B>>,
}

impl<B: Backend> Sequential<B> {
    pub fn new(layers: Vec<Module<B>>) -> Self {
        Self { layers }
    }

    pub fn forward(&self, mut x: Tensor<B>) -> Tensor<B> {
        for layer in &self.layers {
            x = layer.forward(&x);
        }
        x
    }
}

/// Compute simple cross-entropy-like loss for logits.
/// `logits`: [batch, num_classes], `targets`: [batch] (integer labels)
pub fn cross_entropy_loss<B: Backend>(logits: &Tensor<B>, targets: &Tensor<B>) -> Tensor<B> {
    match (logits, targets) {
        (Tensor::D2(l), Tensor::D1(t)) => {
            let [batch, classes] = l.dims();
            let device = l.device();
            let indices = t.clone().int().reshape([batch, 1]).to_device(&device);
            let mut one_hot = burn::tensor::Tensor::<B, 2>::zeros([batch, classes], &device);
            one_hot = one_hot.scatter(1, indices, burn::tensor::Tensor::ones([batch, 1], &device));
            Tensor::D1(burn::tensor::loss::cross_entropy_with_logits(l.clone(), one_hot).reshape([1]))
        }
        _ => panic!("cross_entropy_loss expects D2 logits [batch, classes] and D1 labels [batch]"),
    }
}

/// Compute Mean Squared Error loss.
pub fn mse_loss<B: Backend>(pred: &Tensor<B>, target: &Tensor<B>) -> Tensor<B> {
    match (pred, target) {
        (Tensor::D2(p), Tensor::D2(t)) => {
            let diff = p.clone() - t.clone();
            Tensor::D1(diff.powf_scalar(2.0).mean().reshape([1]))
        }
        (Tensor::D1(p), Tensor::D1(t)) => {
            let diff = p.clone() - t.clone();
            Tensor::D1(diff.powf_scalar(2.0).mean().reshape([1]))
        }
        _ => panic!("mse_loss expects D1 or D2 tensors of same shape"),
    }
}

/// Compute Binary Cross Entropy loss with logits.
pub fn bce_loss<B: Backend>(pred: &Tensor<B>, target: &Tensor<B>) -> Tensor<B> {
    match (pred, target) {
        (Tensor::D2(p), Tensor::D2(t)) => {
            // L = max(p, 0) - p * t + log(1 + exp(-|p|))
            let max_p_0 = p.clone().mask_fill(p.clone().lower_equal_elem(0.0), 0.0);
            let p_abs = p.clone().abs();
            let loss = max_p_0 - p.clone() * t.clone() + (p_abs.neg().exp() + 1.0).log();
            Tensor::D1(loss.mean().reshape([1]))
        }
        _ => panic!("bce_loss expects D2 tensors"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::backend::NdArray;

    type TestBackend = NdArray<f32>;

    #[test]
    fn linear_forward_shape() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let module: Module<TestBackend> = Module::linear(4, 8, true);
        let x = Tensor::<TestBackend>::zeros_2d(2, 4, &device);
        let out = module.forward(&x);
        assert_eq!(out.shape().0, vec![2, 8]);
    }

    #[test]
    fn embedding_forward_shape() {
        // Embedding forward through the Module wrapper is complex because we'd
        // need an integer input. Test the raw Burn embedding directly via Module::new.
        let device: <TestBackend as Backend>::Device = Default::default();
        let config = burn::nn::EmbeddingConfig::new(100, 16);
        let emb = config.init(&device);
        // 3 tokens → [1, 3, 16]
        let ids = burn::tensor::Tensor::<TestBackend, 2, burn::tensor::Int>::from_ints(
            burn::tensor::TensorData::new(vec![0i32, 5, 42], [1, 3]),
            &device,
        );
        let out = emb.forward(ids);
        assert_eq!(out.dims(), [1, 3, 16]);
    }

    #[test]
    fn layer_norm_forward_shape() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let module: Module<TestBackend> = Module::layer_norm(8);
        let x = Tensor::<TestBackend>::zeros_2d(4, 8, &device);
        let out = module.forward(&x);
        assert_eq!(out.shape().0, vec![4, 8]);
    }

    #[test]
    fn sequential_forward_shape() {
        let seq: Sequential<TestBackend> = Sequential::new(vec![
            Module::linear(4, 16, true),
            Module::dropout(0.0),
            Module::linear(16, 8, true),
        ]);
        let device: <TestBackend as Backend>::Device = Default::default();
        let x = Tensor::<TestBackend>::zeros_2d(3, 4, &device);
        let out = seq.forward(x);
        assert_eq!(out.shape().0, vec![3, 8]);
    }

    #[test]
    fn mse_loss_is_zero_for_identical_inputs() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let x = Tensor::<TestBackend>::from_vec_2d(vec![1.0, 2.0, 3.0, 4.0], 2, 2, &device);
        let loss = mse_loss(&x, &x);
        let v = loss.to_vec();
        assert!((v[0]).abs() < 1e-5, "MSE of identical inputs should be ~0, got {}", v[0]);
    }

    #[test]
    fn mse_loss_is_positive_for_different_inputs() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let pred = Tensor::<TestBackend>::from_vec_2d(vec![1.0, 0.0, 0.0, 1.0], 2, 2, &device);
        let tgt  = Tensor::<TestBackend>::zeros_2d(2, 2, &device);
        let loss = mse_loss(&pred, &tgt);
        let v = loss.to_vec();
        assert!(v[0] > 0.0, "MSE should be positive: {}", v[0]);
    }

    #[test]
    fn lora_linear_forward_shape() {
        let module: Module<TestBackend> = Module::lora_linear(4, 8, 2, 1.0);
        let device: <TestBackend as Backend>::Device = Default::default();
        let x = Tensor::<TestBackend>::zeros_2d(3, 4, &device);
        let out = module.forward(&x);
        assert_eq!(out.shape().0, vec![3, 8]);
    }

    #[test]
    fn dropout_forward_shape() {
        let module: Module<TestBackend> = Module::dropout(0.5);
        let device: <TestBackend as Backend>::Device = Default::default();
        let x = Tensor::<TestBackend>::zeros_2d(2, 4, &device);
        let out = module.forward(&x);
        assert_eq!(out.shape().0, vec![2, 4]);
    }

    #[test]
    fn conv1d_forward_shape() {
        let module: Module<TestBackend> = Module::conv1d(3, 6, 2);
        let device: <TestBackend as Backend>::Device = Default::default();
        // Conv1d expects [batch, channels_in, length] -> D3
        let x = burn::tensor::Tensor::<TestBackend, 3>::zeros([2, 3, 10], &device);
        let x_wrapped = Tensor::D3(x);
        let out = module.forward(&x_wrapped);
        // length out = length - kernel_size + 1 = 10 - 2 + 1 = 9
        assert_eq!(out.shape().0, vec![2, 6, 9]);
    }

    #[test]
    fn conv2d_forward_shape() {
        let module: Module<TestBackend> = Module::conv2d(3, 6, 3);
        let device: <TestBackend as Backend>::Device = Default::default();
        // Conv2d expects [batch, channels_in, H, W] -> D4
        let x = burn::tensor::Tensor::<TestBackend, 4>::zeros([2, 3, 10, 10], &device);
        let x_wrapped = Tensor::D4(x);
        let out = module.forward(&x_wrapped);
        // Output H/W = 10 - 3 + 1 = 8
        assert_eq!(out.shape().0, vec![2, 6, 8, 8]);
    }

    #[test]
    fn cross_entropy_loss_valid() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let logits = Tensor::<TestBackend>::zeros_2d(2, 5, &device);
        // targets must be D1Int
        let targets = Tensor::D1Int(burn::tensor::Tensor::<TestBackend, 1, burn::tensor::Int>::from_data(
            burn::tensor::TensorData::new(vec![1, 3], [2]),
            &device
        ));
        let loss = cross_entropy_loss(&logits, &targets);
        assert_eq!(loss.shape().0, vec![1]); // returns scalar [1] wrapped in D1
    }

    #[test]
    fn bce_loss_identical_inputs() {
        let device: <TestBackend as Backend>::Device = Default::default();
        let pred = Tensor::<TestBackend>::zeros_2d(2, 2, &device);
        let tgt = Tensor::<TestBackend>::zeros_2d(2, 2, &device);
        let loss = bce_loss(&pred, &tgt);
        assert_eq!(loss.shape().0, vec![1]);
    }
}
