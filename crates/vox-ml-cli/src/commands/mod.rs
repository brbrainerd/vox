#[cfg(any(feature = "mens-base", feature = "gpu"))]
pub mod mens;

#[cfg(feature = "gpu")]
pub mod schola;

#[cfg(feature = "oratio")]
pub mod oratio_cmd;
#[cfg(feature = "oratio-mic")]
pub mod oratio_mic;

#[cfg(feature = "populi")]
pub mod populi_attest;
#[cfg(feature = "populi")]
pub mod populi_cli;
#[cfg(feature = "populi")]
pub mod populi_join;
#[cfg(feature = "populi")]
pub mod populi_lifecycle;

#[cfg(any(feature = "gpu", feature = "mens-dei", feature = "mens-base"))]
pub mod ai;

#[cfg(feature = "mens-base")]
pub mod corpus;
