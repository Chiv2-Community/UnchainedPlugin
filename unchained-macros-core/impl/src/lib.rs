use proc_macro::TokenStream;

mod common;
mod docs;
mod function;
mod function_detour;
mod function_listener;
mod patch;
mod runtime;
mod selection;
mod signature;
mod value;

/// Register a `fn(u64) -> Patch` factory or immutable `Patch` static.
/// An optional enum variant restricts applicability. Required coverage is grouped
/// by resolver and excludes selections explicitly disabled by that resolver.
#[proc_macro_attribute]

pub fn register_patch(args: TokenStream, item: TokenStream) -> TokenStream {
    patch::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Declare a fieldless selection enum with optional `require = [Variant, ...]`
/// and `fallback = Variant`. The fallback variant itself terminates the chain.
/// Supplies `SelectionKey`, `Clone`, `Copy`, `PartialEq`, and `Eq`.
#[proc_macro_attribute]

pub fn resolver_selection_key(args: TokenStream, item: TokenStream) -> TokenStream {
    selection::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Bind an unsafe Rust method/function to a named resolver, or to
/// `Backend, descriptor` with a `&'static Resolver<Backend>` expression.
///
/// The empty body calls the native entry or its original trampoline. Receiver
/// types and declared per-call lifetime relationships are preserved.
#[proc_macro_attribute]

pub fn bind_resolved_func(args: TokenStream, item: TokenStream) -> TokenStream {
    function::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Register a replacement for a typed method binding, not a resolver.
///
/// Hover the target method in this attribute to inspect its native declaration,
/// resolver link, and original-call behavior. The replacement's hover links
/// back to that binding and explains game-side interception.
#[proc_macro_attribute]

pub fn bind_func_detour(args: TokenStream, item: TokenStream) -> TokenStream {
    function_detour::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[proc_macro_attribute]

pub fn bind_func_entry_listener(args: TokenStream, item: TokenStream) -> TokenStream {
    function_listener::expand(args.into(), item.into(), false)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[proc_macro_attribute]

pub fn bind_func_exit_listener(args: TokenStream, item: TokenStream) -> TokenStream {
    function_listener::expand(args.into(), item.into(), true)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Mark a native type for source documentation, or expose bindings in a concrete impl.
///
/// Type annotations add documentation only. Native global reads and vtable
/// dispatch are explicit operations, not generated method forwarding.
#[proc_macro_attribute]

pub fn runtime_bindings(args: TokenStream, item: TokenStream) -> TokenStream {
    runtime::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Bind native data to an explicit unsafe read.
/// Static pointer bindings expose `read()`: `Value` reloads the pointer slot;
/// `Address` returns inline storage's pointer. Neither form generates native methods.
#[proc_macro_attribute]

pub fn bind_resolved_value(args: TokenStream, item: TokenStream) -> TokenStream {
    value::expand(args.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
