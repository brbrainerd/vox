# Nanopublication conformance fixtures — provenance

These fixtures back `crates/vox-scientia/tests/nanopub_conformance.rs`, which asserts
that `vox_scientia::nanopub::spec::validate_offline` agrees with real, externally
produced signed nanopublication vectors (offline Trusty-hash + RSA-signature check).

## `valid/` — genuine signed nanopublications (must be ACCEPTED)

Source: official upstream suite
**`Nanopublication/nanopub-testsuite`**
<https://github.com/Nanopublication/nanopub-testsuite>

- Commit: `037164433a8cf5fa36fc35c159f8805152aacdd7` (branch `main`)
- Upstream path: `transform/signed/rsa-key1/`
- License: **MIT** (`LICENSE` at repo root)

Each file is the signed output (`<name>.out.trig`) of an upstream transform,
copied verbatim and renamed to `<name>.trig`. They are real RSA-signed
nanopublications (each carries `npx:hasAlgorithm "RSA"`, `npx:hasPublicKey`,
`npx:hasSignature`, `npx:hasSignatureTarget`) and were produced by the upstream
tooling, **not** hand-fabricated.

| local file       | upstream file (`transform/signed/rsa-key1/`) |
|------------------|----------------------------------------------|
| `aida1.trig`     | `aida1.out.trig`                             |
| `example5.trig`  | `example5.out.trig`                          |
| `example6.trig`  | `example6.out.trig`                          |
| `example7.trig`  | `example7.out.trig`                          |
| `example8.trig`  | `example8.out.trig`                          |

All five validate `Ok` under `validate_offline` (verified — see the conformance test).

## `invalid/` — tampered vectors (must be REJECTED)

Each is a copy of one of the valid vectors above with a single-byte mutation that
breaks the Trusty hash / RSA signature. These are legitimate "must be rejected"
cases; they are derived from the MIT-licensed upstream files.

| local file                          | derived from   | mutation                                                         |
|-------------------------------------|----------------|------------------------------------------------------------------|
| `example5-tampered-signature.trig`  | `example5.trig`| flipped one base64 char in `npx:hasSignature` (`nWtF…` → `nWtG…`) |
| `aida1-tampered-assertion.trig`     | `aida1.trig`   | changed assertion text (`Malaria…` → `Smallpox…`)                |
| `example6-tampered-pubkey.trig`     | `example6.trig`| flipped one base64 char in `npx:hasPublicKey` (`MIGf…` → `MIHf…`) |

All three return `Err` ("the hash of the nanopublication is different than the
expected hash") under `validate_offline` (verified — see the conformance test).
