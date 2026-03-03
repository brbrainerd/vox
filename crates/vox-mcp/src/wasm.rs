use anyhow::Result;
use std::path::Path;
use wasmtime::{Engine, Instance, Linker, Module, Store};

/// Encapsulates the Wasm runtime (Wasmtime) for loading and running
/// MCP servers compiled to WebAssembly.
pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    /// Creates a new, default Wasmtime engine runtime.
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    /// Loads a WebAssembly module from a given file path.
    pub fn load_module<P: AsRef<Path>>(&self, path: P) -> Result<WasmModule> {
        let module = Module::from_file(&self.engine, path).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(WasmModule {
            engine: self.engine.clone(),
            module,
        })
    }
}

/// A loaded WebAssembly MCP server module, ready to be instantiated.
pub struct WasmModule {
    engine: Engine,
    module: Module,
}

impl WasmModule {
    /// Instantiates the Wasm module.
    /// This prepares the module for execution and memory interaction.
    pub fn instantiate(&self) -> Result<ActiveWasmInstance> {
        // Create a new store for this instance
        let mut store = Store::new(&self.engine, ());

        // Setup the linker to resolve imports (e.g., logging or host functions can be linked here)
        let linker = Linker::new(&self.engine);

        // Instantiate the module into the store
        let instance = linker.instantiate(&mut store, &self.module).map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(ActiveWasmInstance {
            store,
            instance,
        })
    }
}

/// Represents a running Wasm module instance bound to its execution Store,
/// providing low-level memory interactions conforming to standard Wasm ABI patterns.
pub struct ActiveWasmInstance {
    store: Store<()>,
    instance: Instance,
}

impl ActiveWasmInstance {
    /// Reads a string from the Wasm memory given a pointer and length.
    pub fn read_string(&mut self, ptr: i32, len: i32) -> Result<String> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow::anyhow!("failed to find 'memory' export"))?;

        let data = memory.data(&self.store);
        let start = ptr as usize;
        let end = start + (len as usize);

        if end > data.len() {
            return Err(anyhow::anyhow!("Wasm memory read out of bounds"));
        }

        let slice = &data[start..end];
        let s = std::str::from_utf8(slice)?;
        Ok(s.to_string())
    }

    /// Allocates memory inside the Wasm guest and writes the payload bytes into it,
    /// returning the pointer to the allocated buffer.
    pub fn write_bytes(&mut self, payload: &[u8]) -> Result<i32> {
        // Look up the guest's allocator function (e.g. `allocate` or `malloc`)
        let alloc_fn = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "allocate")
            .map_err(|e| anyhow::anyhow!("'allocate' export not found: {e}"))?;

        // Ask the guest to allocate space
        let len = payload.len() as i32;
        let ptr = alloc_fn
            .call(&mut self.store, len)
            .map_err(|e| anyhow::anyhow!("Wasm allocation failed: {e}"))?;

        // Write the payload into the allocated space
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow::anyhow!("failed to find 'memory' export"))?;

        memory
            .write(&mut self.store, ptr as usize, payload)
            .map_err(|e| anyhow::anyhow!("Wasm memory write failed: {e}"))?;

        Ok(ptr)
    }

    /// Calls the standard MCP execution handler exported by the guest module.
    /// The guest function is expected to accept (ptr, len) of the request bytes
    /// and return an encoded u64 representing (result_ptr, result_len).
    pub fn invoke_mcp(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        let req_len = payload.len() as i32;
        let req_ptr = self.write_bytes(payload)?;

        let handle_fn = self
            .instance
            .get_typed_func::<(i32, i32), u64>(&mut self.store, "handle_mcp_request")
            .map_err(|e| anyhow::anyhow!("'handle_mcp_request' export not found: {e}"))?;

        let result_packed = handle_fn
            .call(&mut self.store, (req_ptr, req_len))
            .map_err(|e| anyhow::anyhow!("Wasm MCP execution failed: {e}"))?;

        let res_ptr = (result_packed >> 32) as i32;
        let res_len = (result_packed & 0xFFFFFFFF) as i32;

        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow::anyhow!("failed to find 'memory' export"))?;

        let mut out_buffer = vec![0u8; res_len as usize];
        memory
            .read(&self.store, res_ptr as usize, &mut out_buffer)
            .map_err(|e| anyhow::anyhow!("failed to read result from Wasm memory: {e}"))?;

        // Optionally, invoke the guest's `deallocate` function here to free both strings
        if let Ok(free_fn) = self.instance.get_typed_func::<(i32, i32), ()>(&mut self.store, "deallocate") {
            let _ = free_fn.call(&mut self.store, (req_ptr, req_len));
            let _ = free_fn.call(&mut self.store, (res_ptr, res_len));
        }

        Ok(out_buffer)
    }
}

/// Task 2: In-Memory Transport Layer
/// Bypasses Stdio/HTTP and routes JSON-RPC messages directly to Wasm.
pub struct InMemTransport {
    pub tx: tokio::sync::mpsc::Sender<String>,
    pub rx: tokio::sync::mpsc::Receiver<String>,
}

impl InMemTransport {
    pub fn new() -> (Self, tokio::sync::mpsc::Receiver<String>, tokio::sync::mpsc::Sender<String>) {
        let (tx1, rx1) = tokio::sync::mpsc::channel(100);
        let (tx2, rx2) = tokio::sync::mpsc::channel(100);
        (
            Self { tx: tx1, rx: rx2 },
            rx1,
            tx2
        )
    }

    /// Task 3: Binary Format Serialization
    /// Demonstrates decoding an MCP message via MessagePack instead of JSON.
    pub fn decode_messagepack<T: serde::de::DeserializeOwned>(payload: &[u8]) -> Result<T> {
        rmp_serde::from_slice(payload).map_err(|e| anyhow::anyhow!("MessagePack decode error: {e}"))
    }

    /// Encodes an MCP message to MessagePack binary format.
    pub fn encode_messagepack<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>> {
        rmp_serde::to_vec(msg).map_err(|e| anyhow::anyhow!("MessagePack encode error: {e}"))
    }
}
