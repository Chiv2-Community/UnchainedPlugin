//! Typed signatures for composing resolver scan modes.

use std::{future::Future, pin::Pin};
use patternsleuth::resolvers::{AsyncContext, ResolveError};

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DefaultResult(pub u64);

impl DefaultResult {
    pub fn offset(&self) -> u64 {
        self.0
    }
}

type OffsetFuture<'a> = Pin<Box<dyn Future<Output = Result<DefaultResult, ResolveError>> + Send + 'a>>;
pub type OffsetCalculator<'a> = std::sync::Arc<
    dyn Fn(&'a AsyncContext<'a>) -> OffsetFuture<'a> + Send + Sync + 'a,
>;

pub struct Signature<'a> {
    pub offset_calculator: OffsetCalculator<'a>,
    pub signature_string: String,
}

impl<'a> Signature<'a> {
    pub async fn calculate_offset(&self, ctx: &'a AsyncContext<'a>) -> Result<DefaultResult, ResolveError> {
        (self.offset_calculator)(ctx).await
    }
}

impl<'a> AsRef<str> for Signature<'a> {
    fn as_ref(&self) -> &str {
        &self.signature_string
    }
}

impl<'a> Clone for Signature<'a> {
    fn clone(&self) -> Self {
        Self {
            offset_calculator: self.offset_calculator.clone(),
            signature_string: self.signature_string.clone(),
        }
    }
}

crate::wrap_process_macro!(Simple);
crate::wrap_process_macro!(Call);
crate::wrap_process_macro!(First);
crate::wrap_process_macro!(XrefLast);
crate::wrap_process_macro!(XrefFirst);
