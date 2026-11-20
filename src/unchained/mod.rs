//! Plugin behavior layered on the native model.

pub mod blueprints;
pub mod patches;
pub(crate) mod startup;

#[cfg(test)]
mod native_bindings_tests;
