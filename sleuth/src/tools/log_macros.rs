
#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[..name.len() - 3]
    }};
}

#[macro_export]
macro_rules! __slog_internal {
    // With flags
    ( $level:ident, $( $flag:ident ),+ ; $($arg:tt)* ) => {{
        // use std::io::Write;

        let mut context_parts = vec![];
        let mut target: Option<String> = None;

        $(
            match stringify!($flag) {
                "f" => {
                    fn extract_segment(s: &str) -> Option<String> {
                        let mut end = s;
                        while let Some(stripped) = end.strip_suffix("::{{closure}}") {
                            end = stripped;
                        }
                        end.rsplit("::").next().map(|s| s.to_string())
                    }
                    let short_name = extract_segment($crate::function!()).unwrap();
                    context_parts.push(format!("{}", short_name));
                    // target = Some(short_name);              
                    target = Some("function".to_string());
                }
                "func" => {
                    target = Some("function".to_string());
                    context_parts.push(format!("{}", $crate::function!()))
                },
                "file" => context_parts.push(file!().to_string()),
                "line" => context_parts.push(format!("L{}", line!())),
                "column" => context_parts.push(format!("C{}", std::column!())),
                "mod"    => context_parts.push(format!("M{}", std::module_path!())),
                _        => {}
            }
        )+
        if let Some(tgt) = target {
            log::$level!(target: &tgt, "[{}] {}", context_parts.join("|"), format_args!($($arg)*));
        }
        else {
            log::$level!("[{}]", context_parts.join("|"));
        }

    }};

    // No flags
    ( $level:ident, $($arg:tt)* ) => {{
        log::$level!(
            "{}:{}:{}",
            $crate::function!(),
            std::line!(),
            std::column!()
        );
        log::$level!($($arg)*);
    }};
}

// FIXME: Nihi: couldn't figure out how to generalize it
//              not possible to make nested macros with repeating patterns?
// #[macro_export]
// macro_rules! generate_slog_macro {
//     ($name:ident, $level:ident) => {
//         #[macro_export]
//         macro_rules! $name {
//             ( $sublevel:ident ; $( $flag:ident ),+ ; $($arg:tt)* ) => {
//                 println![stringify!(sublevel)];
//                 $crate::__slog_internal!($level, $( $flag ),+ ; $($arg)*);
//             };
//             ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
//                 $crate::__slog_internal!($level, $( $flag ),+ ; $($arg)*);
//             };
//             ($($arg:tt)*) => {
//                 $crate::__slog_internal!($level, $($arg)*);
//             };
//         }
//     };
// }

#[macro_export]
macro_rules! sinfo {
    //     ( $(fn|file|line|column|mod)+ ; $($arg:tt)* ) => {
    //         $crate::__slog_internal!(info, $($arg)*);
    //     };
    // ( $sublevel:ident; $( $flag:ident ),+ ; $($arg:tt)* ) => {
    //     println![stringify!(sublevel)];
    //     $crate::__slog_internal!(info, $( $flag ),+ ; $($arg)*);
    // };
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(info, $( $flag ),+ ; $($arg)*)
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(info, $($arg)*)
    };
}

#[macro_export]
macro_rules! swarn {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(warn, $( $flag ),+ ; $($arg)*)
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(warn, $($arg)*)
    };
}

#[macro_export]
macro_rules! sdebug {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(debug, $( $flag ),+ ; $($arg)*)
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(debug, $($arg)*)
    };
}

#[macro_export]
macro_rules! strace {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(trace, $( $flag ),+ ; $($arg)*)
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(trace, $($arg)*)
    };
}

#[macro_export]
macro_rules! serror {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(error, $( $flag ),+ ; $($arg)*)
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(error, $($arg)*)
    };
}

/// ## Example usage
/// ```rust
/// debug_where!();
/// ```
/// ```rust
/// debug_where!("Entering important state {}", state);
/// ```
#[macro_export]
macro_rules! debug_where {
    () => {
        log::debug!(target: "function", "From: {}", $crate::function!());
    };
    ($($arg:tt)*) => {
        log::debug!(
            target: "function",
            "[{}] - {}",
            $crate::function!(),
            format!($($arg)*)
        );
    };
}