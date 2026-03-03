/// Configuration for connecting to a Vox database.
#[derive(Debug, Clone)]
pub enum DbConfig {
    /// Connect to a remote Turso database via HTTP.
    Remote { url: String, token: String },

    /// Open a local file-based database (requires `local` feature).
    #[cfg(feature = "local")]
    Local { path: String },

    /// Open an in-memory database for testing (requires `local` feature).
    #[cfg(feature = "local")]
    Memory,

    /// Open an embedded replica that syncs with a remote primary
    /// (requires `replication` feature).
    #[cfg(feature = "replication")]
    EmbeddedReplica {
        local_path: String,
        url: String,
        token: String,
    },
}

impl DbConfig {
    /// Create a remote config from URL and token.
    pub fn remote(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::Remote {
            url: url.into(),
            token: token.into(),
        }
    }

    /// Create a local file config (requires `local` feature).
    #[cfg(feature = "local")]
    pub fn local(path: impl Into<String>) -> Self {
        Self::Local { path: path.into() }
    }

    /// Create an in-memory config for testing (requires `local` feature).
    #[cfg(feature = "local")]
    pub fn memory() -> Self {
        Self::Memory
    }

    /// Create an embedded replica config (requires `replication` feature).
    #[cfg(feature = "replication")]
    pub fn embedded_replica(
        local_path: impl Into<String>,
        url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self::EmbeddedReplica {
            local_path: local_path.into(),
            url: url.into(),
            token: token.into(),
        }
    }

    /// Read config from VOX_DB_URL, VOX_DB_TOKEN, VOX_DB_PATH.
    pub fn from_env() -> Result<Self, String> {
        let url = std::env::var("VOX_DB_URL").ok();
        let token = std::env::var("VOX_DB_TOKEN").ok();
        let path = std::env::var("VOX_DB_PATH").ok();

        match (url, token, path) {
            (Some(_u), Some(_t), Some(_p)) => {
                #[cfg(feature = "replication")]
                return Ok(Self::embedded_replica(_p, _u, _t));
                #[cfg(not(feature = "replication"))]
                return Err("Embedded replica config requires 'replication' feature".into());
            }
            (Some(u), Some(t), None) => Ok(Self::remote(u, t)),
            (None, None, Some(_p)) => {
                #[cfg(feature = "local")]
                return Ok(Self::local(_p));
                #[cfg(not(feature = "local"))]
                return Err("Local DB config requires 'local' feature".into());
            }
            (None, None, None) => {
                #[cfg(feature = "local")]
                return Ok(Self::memory());
                #[cfg(not(feature = "local"))]
                return Err("Memory DB config requires 'local' feature".into());
            }
            _ => Err("Invalid database configuration in environment variables".into()),
        }
    }
}
