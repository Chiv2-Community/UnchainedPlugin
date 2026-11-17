use std::{future::Future, pin::Pin};
use patternsleuth::resolvers::{AsyncContext, ResolveError};

define_pattern_resolver!(@emit_header DefaultResult);

impl DefaultResult {
    pub fn offset(&self) -> usize {
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

wrap_process_macro!(Simple);
wrap_process_macro!(Call);
wrap_process_macro!(First);
wrap_process_macro!(XrefLast);
wrap_process_macro!(XrefFirst);
