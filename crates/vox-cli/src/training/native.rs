use std::path::Path;
use anyhow::Result;
use owo_colors::OwoColorize;
use vox_tensor::burn;

use vox_tensor::burn::tensor::backend::Backend;
use vox_tensor::burn::module::Module as BurnModule;
use vox_tensor::data::{JsonlDataLoader, VOCAB_SIZE};
use vox_tensor::nn::cross_entropy_loss;
use vox_tensor::optim::{AdamW, LinearWarmupScheduler};
use vox_tensor::tensor::Tensor;
use vox_tensor::burn::prelude::Int;
use vox_tensor::train::{Checkpoint, gradient_clip_norm};

/// A native transformer model for Vox dogfood training.
#[derive(BurnModule, Debug)]
pub struct VoxTransformer<B: Backend> {
    embedding: burn::nn::Embedding<B>,
    blocks: Vec<TransformerBlock<B>>,
    norm: burn::nn::LayerNorm<B>,
    output: burn::nn::Linear<B>,
}

#[derive(BurnModule, Debug)]
struct TransformerBlock<B: Backend> {
    attention: burn::nn::attention::MultiHeadAttention<B>,
    norm1: burn::nn::LayerNorm<B>,
    ff_linear1: burn::nn::Linear<B>,
    ff_linear2: burn::nn::Linear<B>,
    norm2: burn::nn::LayerNorm<B>,
}

impl<B: Backend> VoxTransformer<B> {
    pub fn new(device: &B::Device, vocab_size: usize, d_model: usize, n_heads: usize, n_layers: usize) -> Self {
        let embedding = burn::nn::EmbeddingConfig::new(vocab_size, d_model).init(device);
        let mut blocks = Vec::with_capacity(n_layers);
        for _ in 0..n_layers {
            blocks.push(TransformerBlock::new(device, d_model, n_heads));
        }
        let norm = burn::nn::LayerNormConfig::new(d_model).init(device);
        let output = burn::nn::LinearConfig::new(d_model, vocab_size).init(device);

        Self {
            embedding,
            blocks,
            norm,
            output,
        }
    }

    pub fn forward(&self, x: burn::tensor::Tensor<B, 2, Int>) -> burn::tensor::Tensor<B, 3> {
        let mut h = self.embedding.forward(x);
        for block in &self.blocks {
            h = block.forward(h);
        }
        h = self.norm.forward(h);
        self.output.forward(h)
    }
}

impl<B: Backend> TransformerBlock<B> {
    fn new(device: &B::Device, d_model: usize, n_heads: usize) -> Self {
        let attention = burn::nn::attention::MultiHeadAttentionConfig::new(d_model, n_heads).init(device);
        let norm1 = burn::nn::LayerNormConfig::new(d_model).init(device);
        let norm2 = burn::nn::LayerNormConfig::new(d_model).init(device);

        let ff_linear1 = burn::nn::LinearConfig::new(d_model, d_model * 4).init(device);
        let ff_linear2 = burn::nn::LinearConfig::new(d_model * 4, d_model).init(device);

        Self {
            attention,
            norm1,
            ff_linear1,
            ff_linear2,
            norm2,
        }
    }

    fn forward(&self, x: burn::tensor::Tensor<B, 3>) -> burn::tensor::Tensor<B, 3> {
        let h = self.norm1.forward(x.clone());
        let attn_input = burn::nn::attention::MhaInput::self_attn(h);
        let attn_out = self.attention.forward(attn_input);
        let x = x + attn_out.context;

        let h = self.norm2.forward(x.clone());
        let h = self.ff_linear1.forward(h);
        let h = burn::tensor::activation::gelu(h);
        let h = self.ff_linear2.forward(h);

        x + h
    }
}

pub async fn run_training(data_dir: &Path, output_dir: Option<&Path>) -> Result<()> {
    println!("{}", "🚀 Starting native Vox training (Burn backend)...".green().bold());

    // Define concrete backend
    type BackendType = burn::backend::Wgpu;
    type ADBackend = burn::backend::Autodiff<BackendType>;
    let device = burn::backend::wgpu::WgpuDevice::default();

    // 1. Data Loading
    let train_jsonl = data_dir.join("train.jsonl");
    let sys_prompt = "You are a Vox expert.".to_string();

    // 2. Model & Optimizer Init
    let d_model = 512;
    let n_layers = 12;
    let n_heads = 8;
    let mut model = VoxTransformer::<ADBackend>::new(&device, VOCAB_SIZE, d_model, n_heads, n_layers);

    let mut optim = AdamW::new();
    let initial_lr = 5e-5;
    let mut scheduler = LinearWarmupScheduler::new(1e-7, initial_lr, 100);

    let output_path = output_dir.unwrap_or(data_dir).to_owned();
    std::fs::create_dir_all(&output_path)?;

    println!("  Architecture: {} layers, {} heads, {} hidden", n_layers, n_heads, d_model);
    println!("  Vocab Size:   {}", VOCAB_SIZE);
    println!("  Data:         {}", train_jsonl.display().cyan());

    // 3. Training Loop
    let epochs = 10;
    let mut _global_step = 0u64;

    for epoch in 1..=epochs {
        let mut total_loss = 0.0;
        let mut steps = 0;

        let loader = JsonlDataLoader::open(&train_jsonl, 256, sys_prompt.clone(), 3)?;

        for (input_ids, labels, _pair) in loader {
            let lr = scheduler.step();

            let input_tensor = burn::tensor::Tensor::<ADBackend, 2, Int>::from_data(
                burn::tensor::TensorData::new(input_ids.clone(), [1, input_ids.len()]),
                &device
            );

            let logits = model.forward(input_tensor);
            let [_, seq_len, _] = logits.dims();
            let logits_flat = logits.reshape([seq_len, VOCAB_SIZE]);

            let labels_tensor = burn::tensor::Tensor::<ADBackend, 1, Int>::from_data(
                burn::tensor::TensorData::new(labels.clone(), [labels.len()]),
                &device
            );

            let vox_logits = Tensor::D2(logits_flat);
            let vox_labels = Tensor::D1Int(labels_tensor);
            let loss_tensor = cross_entropy_loss(&vox_logits, &vox_labels);

            let loss_scalar = match loss_tensor {
                Tensor::D1(t) => t.mean(),
                _ => panic!("Expected loss tensor"),
            };

            let loss_val = loss_scalar.clone().into_data().as_slice::<f32>().unwrap()[0];
            total_loss += loss_val;

            let mut grads = loss_scalar.backward();

            // Apply gradient clipping
            gradient_clip_norm::<ADBackend>(&mut grads, 1.0);

            model = optim.step(lr, model, grads);

            steps += 1;
            _global_step += 1;

            if steps % 10 == 0 {
                println!(
                    "  Epoch {:2} | Step {:4} | Loss {:.6} | LR {:.8}",
                    epoch, steps, total_loss / (steps as f32), lr
                );
            }
        }

        let avg_loss = total_loss / (steps.max(1) as f32);
        println!("  {} Epoch {} Summary: Avg Loss {:.6}", "✓".green(), epoch, avg_loss);

        // Save Checkpoint after each epoch
        let checkpoint_name = format!("checkpoint_epoch_{}.bin", epoch);
        let checkpoint_path = output_path.join(checkpoint_name);
        Checkpoint::save(&model, &checkpoint_path).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        println!("  {} Checkpoint saved: {}", "💾".blue(), checkpoint_path.display());
    }

    // final save
    let final_path = output_path.join("model_final.bin");
    Checkpoint::save(&model, &final_path).map_err(|e| anyhow::anyhow!("{:?}", e))?;
    println!("\n{} Native training complete! Final model saved to: {}", "✨".yellow(), final_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_tensor::burn::backend::NdArray;
    use vox_tensor::burn::prelude::Int;
    use vox_tensor::burn::tensor::{Tensor, TensorData};

    type TestBackend = NdArray<f32>;

    #[test]
    fn test_smoke_native_training_forward() {
        // CPU-only smoke test using NdArray backend
        let device = Default::default();
        let vocab_size = 128;
        let d_model = 32;
        let n_heads = 4;
        let n_layers = 2; // small model for test

        let model = VoxTransformer::<TestBackend>::new(&device, vocab_size, d_model, n_heads, n_layers);

        // Dummy input [batch=2, seq_len=10]
        let input_data: Vec<i32> = (0..20).collect();
        let input_tensor = Tensor::<TestBackend, 2, Int>::from_data(
            TensorData::new(input_data, [2, 10]),
            &device,
        );

        let output = model.forward(input_tensor);

        // Output should be [batch, seq_len, vocab_size]
        assert_eq!(output.dims(), [2, 10, vocab_size]);
    }
}
