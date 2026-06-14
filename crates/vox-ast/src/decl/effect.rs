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
    /// Version-control / repository operations (`repo.*` / `vcs.*` builtins).
    Vcs,
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
            "vcs" => Some(Self::Vcs),
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
            Self::Vcs => "vcs",
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

#[cfg(test)]
mod semcov_wave2_tests {
    use super::*;

    #[test]
    fn from_keyword_known_variants() {
        assert_eq!(
            EffectAnnotation::from_keyword("net"),
            Some(EffectAnnotation::Net)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("db"),
            Some(EffectAnnotation::Db)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("fs"),
            Some(EffectAnnotation::Fs)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("env"),
            Some(EffectAnnotation::Env)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("clock"),
            Some(EffectAnnotation::Clock)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("random"),
            Some(EffectAnnotation::Random)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("spawn"),
            Some(EffectAnnotation::Spawn)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("gpu_compute"),
            Some(EffectAnnotation::GpuCompute)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("mutate"),
            Some(EffectAnnotation::Mutate)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("vcs"),
            Some(EffectAnnotation::Vcs)
        );
        assert_eq!(
            EffectAnnotation::from_keyword("nothing"),
            Some(EffectAnnotation::Nothing)
        );
    }

    #[test]
    fn from_keyword_unknown_returns_none() {
        assert_eq!(EffectAnnotation::from_keyword("unknown"), None);
        assert_eq!(EffectAnnotation::from_keyword(""), None);
        assert_eq!(EffectAnnotation::from_keyword("mcp"), None);
        assert_eq!(EffectAnnotation::from_keyword("NET"), None);
    }

    #[test]
    fn as_str_plain_variants() {
        let cases = [
            (EffectAnnotation::Net, "net"),
            (EffectAnnotation::Db, "db"),
            (EffectAnnotation::Fs, "fs"),
            (EffectAnnotation::Env, "env"),
            (EffectAnnotation::Clock, "clock"),
            (EffectAnnotation::Random, "random"),
            (EffectAnnotation::Spawn, "spawn"),
            (EffectAnnotation::GpuCompute, "gpu_compute"),
            (EffectAnnotation::Mutate, "mutate"),
            (EffectAnnotation::Vcs, "vcs"),
            (EffectAnnotation::Nothing, "nothing"),
        ];
        for (variant, expected) in &cases {
            assert_eq!(variant.as_str(), *expected);
        }
    }

    #[test]
    fn as_str_mcp_returns_mcp_regardless_of_tool_name() {
        let e = EffectAnnotation::Mcp("browser".to_string());
        assert_eq!(e.as_str(), "mcp");
    }

    #[test]
    fn display_plain_variants() {
        assert_eq!(EffectAnnotation::Net.to_string(), "net");
        assert_eq!(EffectAnnotation::Nothing.to_string(), "nothing");
        assert_eq!(EffectAnnotation::GpuCompute.to_string(), "gpu_compute");
    }

    #[test]
    fn display_mcp_includes_tool_name() {
        let e = EffectAnnotation::Mcp("read_file".to_string());
        assert_eq!(e.to_string(), "mcp(read_file)");
    }

    #[test]
    fn from_keyword_then_as_str_identity() {
        let keywords = [
            "net",
            "db",
            "fs",
            "env",
            "clock",
            "random",
            "spawn",
            "gpu_compute",
            "mutate",
            "vcs",
            "nothing",
        ];
        for kw in &keywords {
            let variant = EffectAnnotation::from_keyword(kw).unwrap();
            assert_eq!(variant.as_str(), *kw, "round-trip failed for {kw}");
        }
    }
}
