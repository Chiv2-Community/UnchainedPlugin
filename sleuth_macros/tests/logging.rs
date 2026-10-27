extern crate self as patternsleuth;

pub mod resolvers {
    pub use crate::impl_resolver_singleton;
}

// Capture the actual proc-macro output before patternsleuth consumes its body.
#[macro_export]
macro_rules! impl_resolver_singleton {
    ($($tokens:tt)*) => { const EXPANSION: &str = stringify!($($tokens)*); };
}

mod required {
    #[sleuth_macros::resolver(Custom)]
    fn Required() { Err("injected failure") }

    #[test]
    fn uses_shared_error_macro() {
        let compact: String = EXPANSION.chars().filter(|ch| !ch.is_whitespace()).collect();
        assert!(compact.contains("crate::serror!"), "{EXPANSION}");
        assert!(!compact.contains("log::"), "{EXPANSION}");
    }
}

mod optional {
    #[sleuth_macros::resolver(Custom)]
    #[optional]
    fn Optional() { Err("injected failure") }

    #[test]
    fn uses_shared_debug_macro() {
        let compact: String = EXPANSION.chars().filter(|ch| !ch.is_whitespace()).collect();
        assert!(compact.contains("crate::sdebug!"), "{EXPANSION}");
        assert!(!compact.contains("log::"), "{EXPANSION}");
    }
}
