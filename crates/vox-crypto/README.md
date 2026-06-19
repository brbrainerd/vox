# vox-crypto

Pure-Rust cryptographic primitives extracted from the [Vox](https://github.com/vox-foundation/vox) project.

## Algorithms

| Primitive | Crate |
|-----------|-------|
| AEAD encryption | `chacha20poly1305` |
| Signatures | `ed25519-dalek` |
| Key agreement | `x25519-dalek` |
| Hashing | `blake3`, `sha3`, `xxhash-rust` |
| Encoding | `hex`, `zeroize` |

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
vox-crypto = "0.6"
```

## License

Apache-2.0
