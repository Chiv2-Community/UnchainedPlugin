// Fixture-authored accessors keep runtime tests inside the generated helper scope.
macro_rules! exec_access {
    ($resolver:ident, $binding:ident, $dispatch:ident) => {
        #[allow(non_upper_case_globals)]
        const $resolver: &'static unchained_macros_core::Resolver<unchained_macros_kismet::KismetExecResolver> = &$resolver;
        #[allow(non_upper_case_globals)]
        const $binding: &'static unchained_macros_core::FunctionBinding<unchained_macros_kismet::KismetExecResolver> = &$binding;
        #[allow(non_upper_case_globals)]
        const $dispatch: unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void) = $dispatch;
    };
}
