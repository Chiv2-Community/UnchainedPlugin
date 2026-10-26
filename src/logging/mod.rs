pub mod console;
pub mod log4rs;

mod macros;
#[doc(hidden)]
pub use log as __log;
#[cfg(test)]
mod tests;
pub use crate::{__unchained_debug as debug, __unchained_error as error,
    __unchained_info as info, __unchained_trace as trace, __unchained_warn as warn};

#[doc(hidden)]
pub fn function_name<T>(_: T) -> &'static str {
    // The local marker belongs to the call site, including inside async/closures.
    std::any::type_name::<T>().strip_suffix("::__unchained_function_marker")
        .unwrap_or("???").rsplit("::")
        .find(|segment| *segment != "{{closure}}")
        .unwrap_or("???")
}
