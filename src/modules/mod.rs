//! Register bundled modules and expose their explicit integration boundaries.

mod catalog;
pub(crate) mod deployment;
pub(crate) mod docker;
pub(crate) mod kubernetes;
pub(crate) mod terminal;

pub(crate) use catalog::{MODULES, ModuleId, Origin};

pub(crate) mod git;
pub(crate) mod pipelines;
