pub mod candle_cpu;
pub mod candle_cuda;
pub mod candle_metal;
pub mod llama_cpp_rpc;
pub mod ollama_subprocess;

pub use candle_cpu::CandleCpuBackend;
pub use candle_cuda::CandleCudaBackend;
pub use candle_metal::CandleMetalBackend;
pub use llama_cpp_rpc::LlamaCppRpcBackend;
pub use ollama_subprocess::OllamaSubprocessBackend;
