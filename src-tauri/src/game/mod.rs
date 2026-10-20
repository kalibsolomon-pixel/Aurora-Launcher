//! Loader-neutral game composition.
//!
//! [`plan`] owns the shared composed-plan types every loader family
//! contributes to and every consumer reads. Loader-specific planning and
//! family composers stay in their own adapters (`fabric`, `neoforge`, and
//! future families); this module never parses loader metadata.

pub mod plan;
