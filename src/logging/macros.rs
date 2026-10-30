// Keep filtering, argument evaluation, metadata and KV capture in the log facade.
#[doc(hidden)]
#[macro_export]
macro_rules! __unchained_log {
    ($level:ident, logger: $logger:expr, target: $target:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@dispatch $level, $logger, $target, $($args)+)
    };
    ($level:ident, logger: $logger:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@dispatch $level, $logger, module_path!(), $($args)+)
    };
    ($level:ident, target: $target:expr, $($args:tt)+) => {
        $crate::__unchained_log!(@dispatch $level, $crate::logging::__log::logger(), $target, $($args)+)
    };
    ($level:ident, $($args:tt)+) => {
        $crate::__unchained_log!(@dispatch $level, $crate::logging::__log::logger(), module_path!(), $($args)+)
    };
    (@dispatch $level:ident, $logger:expr, $target:expr, $($args:tt)+) => {
        $crate::__unchained_inline!(@detect [$level, $logger, $target] $($args)+)
    };
    (@emit $level:ident, $logger:expr, $target:expr, fn; $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::__unchained_log!(@emit $level, $logger, $target,
            "[{}] {}", $crate::logging::function_name(__unchained_function_marker),
            format_args!($($args)+))
    }};
    (@emit $level:ident, $logger:expr, $target:expr,
        $($key:tt $(:$capture:tt)? $(= $value:expr)?),+; $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::logging::__log::log!(logger: $logger, target: $target, $crate::logging::__log::Level::$level,
            enclosing_function = $crate::logging::function_name(__unchained_function_marker),
            $($key $(:$capture)? $(= $value)?),+; $($args)+)
    }};
    (@emit $level:ident, $logger:expr, $target:expr, $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::logging::__log::log!(logger: $logger, target: $target, $crate::logging::__log::Level::$level,
            enclosing_function = $crate::logging::function_name(__unchained_function_marker);
            $($args)+)
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __unchained_inline {
    // An explicit first segment distinguishes inline calls from ordinary format arguments.
    (@detect $context:tt fn; $($kind:ident)? ($($args:tt)*)
        $(, $segment:tt $(($($tail:tt)*))?)* $(,)?) => {{
        fn __unchained_function_marker() {}
        $crate::__unchained_inline!(@finish $context []
            [$crate::logging::inline::Segment::Text(format_args!("[{}] ",
                $crate::logging::function_name(__unchained_function_marker))),
             $crate::__unchained_inline!(@segment $($kind)? ($($args)*))
             $(, $crate::__unchained_inline!(@segment $segment $(($($tail)*))?))*])
    }};
    (@detect $context:tt $($key:tt $(:$capture:tt)? $(= $value:expr)?),+;
        $($kind:ident)? ($($args:tt)*) $(, $segment:tt $(($($tail:tt)*))?)* $(,)?) => {
        $crate::__unchained_inline!(@finish $context
            [$($key $(:$capture)? $(= $value)?,)+]
            [$crate::__unchained_inline!(@segment $($kind)? ($($args)*))
             $(, $crate::__unchained_inline!(@segment $segment $(($($tail)*))?))*])
    };
    (@detect $context:tt $($kind:ident)? ($($args:tt)*)
        $(, $segment:tt $(($($tail:tt)*))?)* $(,)?) => {
        $crate::__unchained_inline!(@finish $context []
            [$crate::__unchained_inline!(@segment $($kind)? ($($args)*))
             $(, $crate::__unchained_inline!(@segment $segment $(($($tail)*))?))*])
    };
    (@detect [$level:ident, $logger:expr, $target:expr] $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $logger, $target, $($args)+)
    };
    (@segment palette($name:expr)) => { $crate::logging::inline::Segment::Palette($name) };
    (@segment palette()) => { $crate::logging::inline::Segment::Palette("") };
    (@segment $text:literal) => { $crate::logging::inline::Segment::Text(format_args!($text)) };
    (@segment ($($args:tt)+)) => { $crate::logging::inline::Segment::Text(format_args!($($args)+)) };
    (@finish [$level:ident, $logger:expr, $target:expr] [$($kv:tt)*] [$($segment:expr),+]) => {{
        let level = $crate::logging::__log::Level::$level;
        if level <= $crate::logging::__log::STATIC_MAX_LEVEL
            && level <= $crate::logging::__log::max_level() {
            let logger = &$logger;
            let target = $target;
            if $crate::logging::__log::Log::enabled(&logger,
                &$crate::logging::__log::Metadata::builder().level(level).target(target).build()) {
                match &[$($segment),+] {
                    segments => {
                        fn __unchained_function_marker() {}
                        let id = $crate::logging::inline::next_id();
                        $crate::logging::__log::log!(logger: logger, target: target, level,
                            $($kv)*
                            enclosing_function = $crate::logging::function_name(__unchained_function_marker),
                            __unchained_inline = id;
                            "{}", $crate::logging::inline::Message(segments, id));
                    }
                }
            }
        }
    }};
}

#[macro_export]
macro_rules! sinfo {
    ($($args:tt)+) => { $crate::__unchained_log!(Info, $($args)+) };
}
#[macro_export]
macro_rules! swarn {
    ($($args:tt)+) => { $crate::__unchained_log!(Warn, $($args)+) };
}
#[macro_export]
macro_rules! serror {
    ($($args:tt)+) => { $crate::__unchained_log!(Error, $($args)+) };
}
#[macro_export]
macro_rules! sdebug {
    ($($args:tt)+) => { $crate::__unchained_log!(Debug, $($args)+) };
}
#[macro_export]
macro_rules! strace {
    ($($args:tt)+) => { $crate::__unchained_log!(Trace, $($args)+) };
}
