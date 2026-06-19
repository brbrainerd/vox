//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.

pub mod signature;

pub use signature::{hamming, jaccard_estimate, minhash, shingle, simhash64, tokenize, Signature};
