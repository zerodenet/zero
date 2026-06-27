//! Protocol runtime integration glue.
//!
//! Generic `runtime` owns lifecycle, routing, pipes, and dispatch. This module
//! keeps the remaining UDP integration facades while protocol-private managers,
//! codecs, and pools continue moving behind protocol-local capability code.

pub(crate) mod udp;
