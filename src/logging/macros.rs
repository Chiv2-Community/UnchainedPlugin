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
        $crate::__unchained_inline!(@detect [$level, $logger, $target] [$($args)+] $($args)+)
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
    // Probe with expressions, but parse the original tokens so format_args!
    // retains named arguments rather than receiving opaque assignment expressions.
    (@detect $context:tt [$($raw:tt)+] fn; $format:literal $(, $arg:expr)* $(,)?; $($rest:tt)+) => {
        $crate::__unchained_inline!(@start $context $($raw)+)
    };
    (@detect $context:tt [$($raw:tt)+]
        $($key:tt $(:$capture:tt)? $(= $value:expr)?),+;
        $format:literal $(, $arg:expr)* $(,)?; $($rest:tt)+) => {
        $crate::__unchained_inline!(@start $context $($raw)+)
    };
    (@detect $context:tt [$($raw:tt)+] $format:literal $(, $arg:expr)* $(,)?; $($rest:tt)+) => {
        $crate::__unchained_inline!(@start $context $($raw)+)
    };
    (@detect [$level:ident, $logger:expr, $target:expr] [$($raw:tt)+] $($args:tt)+) => {
        $crate::__unchained_log!(@emit $level, $logger, $target, $($raw)+)
    };
    (@start [$level:ident, $logger:expr, $target:expr] fn; $($args:tt)+) => {{
        fn __unchained_function_marker() {}
        $crate::__unchained_inline!(@parse [$level, $logger, $target; plain; ]
            [Text(format_args!("[{}] ",
                $crate::logging::function_name(__unchained_function_marker))),] [] $($args)+)
    }};
    (@start [$level:ident, $logger:expr, $target:expr]
        $first:literal $(:$first_capture:tt)? = $first_value:expr
        $(, $key:tt $(:$capture:tt)? $(= $value:expr)?)*; $($args:tt)+) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; plain;
            $first $(:$first_capture)? = $first_value,
            $($key $(:$capture)? $(= $value)?,)*] [] [] $($args)+)
    };
    (@start [$level:ident, $logger:expr, $target:expr] $format:literal $($args:tt)*) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; plain; ] [] [] $format $($args)*)
    };
    (@start [$level:ident, $logger:expr, $target:expr]
        $($key:tt $(:$capture:tt)? $(= $value:expr)?),+; $($args:tt)+) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; plain;
            $($key $(:$capture)? $(= $value)?),+,] [] [] $($args)+)
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [$($text:tt)+] ; $($rest:tt)+) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; $mode; $($kv)*]
            [$($kind($value),)* Text(format_args!($($text)+)),]
            [] $($rest)+)
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [] palette($name:expr); $($rest:tt)+) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; styled; $($kv)*]
            [$($kind($value),)* Palette($name),] [] $($rest)+)
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [] palette(); $($rest:tt)+) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; styled; $($kv)*]
            [$($kind($value),)* Palette(""),] [] $($rest)+)
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [$($text:tt)*] ; ) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; $mode; $($kv)*]
            [$($kind($value),)*] [$($text)*])
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [] palette($name:expr) $(;)?) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; styled; $($kv)*]
            [$($kind($value),)* Palette($name),] [""])
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [] palette() $(;)?) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; styled; $($kv)*]
            [$($kind($value),)* Palette(""),] [""])
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [$($text:tt)*] $next:tt $($rest:tt)*) => {
        $crate::__unchained_inline!(@parse [$level, $logger, $target; $mode; $($kv)*]
            [$($kind($value),)*] [$($text)* $next] $($rest)*)
    };
    (@parse [$level:ident, $logger:expr, $target:expr; $mode:ident; $($kv:tt)*] [$($kind:ident($value:expr),)*]
        [$($text:tt)+]) => {
        $crate::__unchained_inline!(@finish [$level, $logger, $target; $mode; $($kv)*]
            [$($kind($value),)* Text(format_args!($($text)+)),])
    };
    (@slot $args:expr) => { "{}" };
    (@finish [$level:ident, $logger:expr, $target:expr; plain; $($kv:tt)*] [$(Text($args:expr),)*]) => {{
        fn __unchained_function_marker() {}
        $crate::logging::__log::log!(logger: $logger, target: $target, $crate::logging::__log::Level::$level,
            $($kv)* enclosing_function = $crate::logging::function_name(__unchained_function_marker);
            concat!($($crate::__unchained_inline!(@slot $args)),*), $($args),*);
    }};
    (@finish [$level:ident, $logger:expr, $target:expr; styled; $($kv:tt)*] [$($kind:ident($value:expr),)*]) => {{
        let level = $crate::logging::__log::Level::$level;
        if level <= $crate::logging::__log::STATIC_MAX_LEVEL
            && level <= $crate::logging::__log::max_level() {
            let logger = &$logger;
            let target = $target;
            if $crate::logging::__log::Log::enabled(&logger,
                &$crate::logging::__log::Metadata::builder().level(level).target(target).build()) {
                match &[$($crate::logging::inline::Segment::$kind($value),)*] {
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
