//! Neutral UDP association runtime glue.
//!
//! The root stays as a facade over the association contract and shared event
//! loop so proxy does not regrow protocol-specific association handling here.

mod contract;
mod lifecycle;

pub(crate) use contract::UdpAssociationLoopRequest;
pub(crate) use lifecycle::run_udp_association_loop;
