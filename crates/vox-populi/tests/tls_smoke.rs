//! P0-T5 acceptance: `vox populi serve --tls cert.pem` accepts an HTTPS peer.

#![cfg(feature = "tls")]

#[tokio::test]
async fn rustls_acceptor_accepts_https_handshake() {
    use vox_populi::tls::{TlsMinVersion, TlsOptions, build_acceptor};

    // Generate a self-signed cert in a tempdir.
    let dir = tempfile::tempdir().unwrap();
    let cert_pem = dir.path().join("cert.pem");
    let key_pem = dir.path().join("key.pem");
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    std::fs::write(&cert_pem, cert.cert.pem()).unwrap();
    std::fs::write(&key_pem, cert.key_pair.serialize_pem()).unwrap();

    let acceptor = build_acceptor(&TlsOptions {
        cert_path: cert_pem,
        key_path: key_pem,
        min_version: TlsMinVersion::V1_3,
    })
    .expect("acceptor built");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let _tls = acceptor.accept(sock).await.expect("server accept");
    });

    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(client_config_skip_verify());
    let domain = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let _client_tls = connector
        .connect(domain, stream)
        .await
        .expect("client handshake");

    server.await.unwrap();
}

fn client_config_skip_verify() -> std::sync::Arc<rustls::ClientConfig> {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, Error};

    #[derive(Debug)]
    struct NoVerify;
    impl ServerCertVerifier for NoVerify {
        fn verify_server_cert(
            &self,
            _: &CertificateDer<'_>,
            _: &[CertificateDer<'_>],
            _: &ServerName<'_>,
            _: &[u8],
            _: UnixTime,
        ) -> Result<ServerCertVerified, Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    let cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(NoVerify))
        .with_no_client_auth();
    std::sync::Arc::new(cfg)
}
