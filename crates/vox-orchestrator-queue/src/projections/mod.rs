//! Concrete [`Projection`](crate::projection::Projection) implementations.

pub mod affinity;
pub mod capabilities;
pub mod kudos;
pub mod locks;

pub use affinity::AffinityProjection;
pub use capabilities::CapabilityProjection;
pub use kudos::KudosProjection;
pub use locks::LocksProjection;
