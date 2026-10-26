// Keep filtering, argument evaluation, metadata and KV capture in the log facade.
#[doc(hidden)]
#[macro_export]
macro_rules! __unchained_log {
    ($level:ident, logger: $logger:expr, target: $target:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $logger, $target, $($args)+)
    };
    ($level:ident, logger: $logger:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $logger, module_path!(), $($args)+)
    };
    ($level:ident, target: $target:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $crate::logging::__log::logger(), $target, $($args)+)
    };
    ($level:ident, $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $crate::logging::__log::logger(), module_path!(), $($args)+)
    };
    (@emit $level:ident, $logger:expr, $target:expr,
        $($key:tt $(:$capture:tt)? $(= $value:expr)?),+; $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::logging::__log::log!(logger: $logger, target: $target, $crate::logging::__log::Level::$level,
            __unchained_function = $crate::logging::function_name(__unchained_function_marker),
            $($key $(:$capture)? $(= $value)?),+; $($args)+)
    }};
    (@emit $level:ident, $logger:expr, $target:expr, $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::logging::__log::log!(logger: $logger, target: $target, $crate::logging::__log::Level::$level,
            __unchained_function = $crate::logging::function_name(__unchained_function_marker);
            $($args)+)
    }};
}

#[macro_export]
macro_rules! info {
    ($($args:tt)+) => { $crate::__unchained_log!(Info, $($args)+) };
}
#[macro_export]
macro_rules! warn {
    ($($args:tt)+) => { $crate::__unchained_log!(Warn, $($args)+) };
}
#[macro_export]
macro_rules! error {
    ($($args:tt)+) => { $crate::__unchained_log!(Error, $($args)+) };
}
#[macro_export]
macro_rules! debug {
    ($($args:tt)+) => { $crate::__unchained_log!(Debug, $($args)+) };
}
#[macro_export]
macro_rules! trace {
    ($($args:tt)+) => { $crate::__unchained_log!(Trace, $($args)+) };
}
