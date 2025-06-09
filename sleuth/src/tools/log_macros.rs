pub mod slog_flags {
    pub const FN: &str = "fn";
    pub const FILE: &str = "file";
    pub const LINE: &str = "line";
    pub const COLUMN: &str = "column";
    pub const MOD: &str = "mod";
}

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
                "func" => context_parts.push(format!("{}", $crate::function!())),
                "file" => context_parts.push(file!().to_string()),
                "line" => context_parts.push(format!("line {}", line!())),
                "column" => context_parts.push(format!("col {}", std::column!())),
                "mod"    => context_parts.push(format!("{}", std::module_path!())),
                _        => {}
            }
        )+

        if !context_parts.is_empty() {
            log::$level!("[{}]", context_parts.join(" | "));
        }

        log::$level!($($arg)*);
    }};

    // No flags
    ( $level:ident, $($arg:tt)* ) => {{
        // log::$level!(
        //     "{} (in {} [{}:{}:{}])",
        //     $crate::function!(),
        //     std::module_path!(),
        //     std::file!(),
        //     std::line!(),
        //     std::column!()
        // );
        log::$level!(
            "{}:{}:{}",
            $crate::function!(),
            std::line!(),
            std::column!()
        );
        log::$level!($($arg)*);
    }};
}

// #[macro_export]
// macro_rules! sinfo {
//     ( $(fn|file|line|column|mod)+ ; $($arg:tt)* ) => {
//         $crate::__slog_internal!(info, $($arg)*);
//     };
//     ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
//         $crate::__slog_internal!(info, $( $flag ),+ ; $($arg)*);
//     };
//     ($($arg:tt)*) => {
//         $crate::__slog_internal!(info, $($arg)*);
//     };
// }




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

// FIXME: Nihi: couldn't figure out how to generalize it
//              not possible to make nested macros with repeating patterns?
// Helper macro to define one macro named $fun that prints with format + args
// macro_rules! nested {
//     (($($f:ident),*) $args:tt) => {
//         println!["asdf"];
//         $(nested!(@call $f $args);)*
//         println!["asdf over"];
//     };
//     (@call $f:ident ($($arg:expr),*)) => {
//             println![stringify![$f]];
//             println![$($arg),*];
//             println!["asdfg over"];
//     };
//     () => {
//         println!["no match"];
//     }
// }

#[macro_export]
macro_rules! sinfo {
    //     ( $(fn|file|line|column|mod)+ ; $($arg:tt)* ) => {
    //         $crate::__slog_internal!(info, $($arg)*);
    //     };
    ( $sublevel:ident; $( $flag:ident ),+ + $(;)? $($arg:tt)* ) => {
        println![stringify!(sublevel)];
        $crate::__slog_internal!(info, $( $flag ),+ ; $($arg)*);
    };
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(info, $( $flag ),+ ; $($arg)*);
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(info, $($arg)*);
    };
}

#[macro_export]
macro_rules! sdebug {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(debug, $( $flag ),+ ; $($arg)*);
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(debug, $($arg)*);
    };
}
#[macro_export]
macro_rules! strace {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(trace, $( $flag ),+ ; $($arg)*);
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(trace, $($arg)*);
    };
}
#[macro_export]
macro_rules! serror {
    ( $( $flag:ident ),+ ; $($arg:tt)* ) => {
        $crate::__slog_internal!(error, $( $flag ),+ ; $($arg)*);
    };
    ($($arg:tt)*) => {
        $crate::__slog_internal!(error, $($arg)*);
    };
}