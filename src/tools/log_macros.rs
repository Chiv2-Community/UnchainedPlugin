
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
                }
                "func" => {
                    context_parts.push(format!("{}", $crate::function!()))
                },
                "file" => context_parts.push(file!().to_string()),
                "line" => context_parts.push(format!("L{}", line!())),
                "column" => context_parts.push(format!("C{}", std::column!())),
                "mod"    => context_parts.push(format!("M{}", std::module_path!())),
                _        => {}
            }
        )+
        log::$level!(target: module_path!(), "[{}] {}", context_parts.join("|"), format_args!($($arg)*));

    }};

    // No flags
    ( $level:ident, $($arg:tt)* ) => {{
        log::$level!(
            target: module_path!(),
            "[{}:{}:{}] {}",
            $crate::function!(),
            std::line!(),
            std::column!(),
            format_args!($($arg)*)
        );
        // log::$level!($($arg)*);
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
/// use UnchainedPlugin::debug_where;
/// # use UnchainedPlugin::function;
/// # fn main() {
/// debug_where!();
/// # }
/// ```
/// ```rust
/// use UnchainedPlugin::debug_where;
/// # use UnchainedPlugin::function;
/// # fn main() {
/// let state = "initializing";
/// debug_where!("Entering important state {}", state);
/// # }
/// ```
#[macro_export]
macro_rules! debug_where {
    () => {
        log::debug!(target: module_path!(), "From: {}", $crate::function!());
    };
    ($($arg:tt)*) => {
        log::debug!(
            target: module_path!(),
            "[{}] - {}",
            $crate::function!(),
            format!($($arg)*)
        );
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    struct Capture(Mutex<Vec<(String, String)>>);
    static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool { true }
        fn log(&self, record: &log::Record) {
            if record.target().starts_with(module_path!()) {
                self.0.lock().unwrap().push((record.target().into(), record.args().to_string()));
            }
        }
        fn flush(&self) {}
    }

    mod child {
        pub fn emit() { crate::sinfo!(f; "child message"); }
    }

    #[test]
    fn helpers_use_calling_module_for_every_level_and_flag() {
        log::set_logger(&CAPTURE).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
        crate::sinfo!("plain info");
        crate::swarn!("plain warning");
        crate::serror!("plain error");
        crate::sdebug!("plain debug");
        crate::strace!("plain trace");
        crate::sinfo!(f; "short function");
        crate::swarn!(func; "full function");
        crate::serror!(file, line, column, mod; "source context");
        crate::sdebug!(f, line; "debug context");
        crate::strace!(func, mod; "trace context");
        crate::debug_where!();
        crate::debug_where!("state: {}", "ready");
        child::emit();

        let records = CAPTURE.0.lock().unwrap();
        assert_eq!(records.len(), 13);
        for (target, _) in &records[..12] {
            assert_eq!(target, module_path!());
        }
        assert!(records[5].1.contains("helpers_use_calling_module_for_every_level_and_flag"));
        assert_eq!(records[12].0, concat!(module_path!(), "::child"));
    }
}
