//! P0-T5: optional rustls acceptor for the populi HTTP plane.
//!
//! Gated behind the `tls` feature. When the feature is on AND the operator
//! provides cert/key paths in `[mesh.transport]`, the server terminates TLS
//! locally; otherwise it runs plain HTTP (existing behaviour).

use std::path::Path;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use thiserror::Error;
use tokio_rustls::TlsAcceptor;

/// Error constructing or using a TLS acceptor.
#[derive(Debug, Error)]
pub enum TlsError {
    /// Failed to read the certificate PEM file.
    #[error("read cert {path}: {source}")]
    ReadCert {
        /// Certificate file path.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read the private key PEM file.
    #[error("read key {path}: {source}")]
    ReadKey {
        /// Key file path.
        path: std::path::PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// File exists but contains no valid PEM blocks.
    #[error("invalid PEM in {path}")]
    InvalidPem {
        /// File with invalid PEM content.
        path: std::path::PathBuf,
    },
    /// rustls rejected the configuration.
    #[error("rustls config: {0}")]
    Rustls(String),
}

/// Options for building a TLS acceptor from operator-supplied cert/key paths.
pub struct TlsOptions {
    /// Path to the PEM-encoded TLS certificate.
    pub cert_path: std::path::PathBuf,
    /// Path to the PEM-encoded private key.
    pub key_path: std::path::PathBuf,
    /// Minimum TLS version to accept.
    pub min_version: TlsMinVersion,
}

/// Minimum TLS protocol version for the acceptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMinVersion {
    /// TLS 1.2 (legacy clients).
    V1_2,
    /// TLS 1.3 (default; recommended).
    V1_3,
}

impl TlsMinVersion {
    /// Parse from an optional string label (`"1.2"` or `"1.3"`; anything else → `V1_3`).
    pub fn parse(s: Option<&str>) -> Self {
        match s.unwrap_or("1.3") {
            "1.2" => TlsMinVersion::V1_2,
            _ => TlsMinVersion::V1_3,
        }
    }
}

/// Build a [`TlsAcceptor`] from operator-supplied PEM cert and key files.
pub fn build_acceptor(opts: &TlsOptions) -> Result<TlsAcceptor, TlsError> {
    let certs = load_certs(&opts.cert_path)?;
    let key = load_key(&opts.key_path)?;
    let cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| TlsError::Rustls(e.to_string()))?;
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

fn load_certs(path: &Path) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, TlsError> {
    let pem = std::fs::read(path).map_err(|source| TlsError::ReadCert {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = std::io::Cursor::new(pem);
    let certs: Result<Vec<_>, _> = rustls_pemfile::certs(&mut reader).collect();
    let certs = certs.map_err(|_| TlsError::InvalidPem {
        path: path.to_path_buf(),
    })?;
    if certs.is_empty() {
        return Err(TlsError::InvalidPem {
            path: path.to_path_buf(),
        });
    }
    Ok(certs)
}

fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, TlsError> {
    let pem = std::fs::read(path).map_err(|source| TlsError::ReadKey {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = std::io::Cursor::new(pem);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|_| TlsError::InvalidPem {
            path: path.to_path_buf(),
        })?
        .ok_or_else(|| TlsError::InvalidPem {
            path: path.to_path_buf(),
        })?;
    Ok(key)
}
