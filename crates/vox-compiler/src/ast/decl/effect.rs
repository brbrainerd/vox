/// Effect annotations for the `uses` clause: `fn f() uses net, db { … }`.
///
/// A missing `uses` clause leaves the function unannotated (open/unconstrained).
/// `uses nothing` declares the function pure; equivalent to `@pure`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum EffectAnnotation {
    /// Outbound HTTP / WebSocket.
    Net,
    /// Database reads or writes.
    Db,
    /// Filesystem reads or writes.
    Fs,
    /// Environment variable reads.
    Env,
    /// Reads current time.
    Clock,
    /// Consumes entropy.
    Random,
    /// Spawns a subprocess or background task.
    Spawn,
    /// GPU compute (kernels / Candle CUDA / inference dispatch).
    GpuCompute,
    /// In-place mutable tensor / optimizer state updates (training).
    Mutate,
    /// Calls a specific MCP tool: `mcp(tool_name)`.
    Mcp(String),
    /// Explicit `uses nothing` — equivalent to `@pure`.
    Nothing,
}

impl EffectAnnotation {
    pub fn from_keyword(s: &str) -> Option<Self> {
        match s {
            "net" => Some(Self::Net),
            "db" => Some(Self::Db),
            "fs" => Some(Self::Fs),
            "env" => Some(Self::Env),
            "clock" => Some(Self::Clock),
            "random" => Some(Self::Random),
            "spawn" => Some(Self::Spawn),
            "gpu_compute" => Some(Self::GpuCompute),
            "mutate" => Some(Self::Mutate),
            "nothing" => Some(Self::Nothing),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Net => "net",
            Self::Db => "db",
            Self::Fs => "fs",
            Self::Env => "env",
            Self::Clock => "clock",
            Self::Random => "random",
            Self::Spawn => "spawn",
            Self::GpuCompute => "gpu_compute",
            Self::Mutate => "mutate",
            Self::Mcp(_) => "mcp",
            Self::Nothing => "nothing",
        }
    }
}

impl std::fmt::Display for EffectAnnotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mcp(tool) => write!(f, "mcp({tool})"),
            other => write!(f, "{}", other.as_str()),
        }
    }
}
