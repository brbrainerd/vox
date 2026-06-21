# vox-secrets (Clavis)

Central secret resolution and OS-keyring vault extracted from the [Vox](https://github.com/vox-foundation/vox) project.

Clavis provides a unified interface for resolving secrets from multiple backends:
- OS keyring (via `keyring`)
- Environment variables
- Encrypted vault files (ChaCha20-Poly1305 via `vox-crypto`)
- Infisical / HashiCorp Vault (feature-gated)

## License

Apache-2.0
