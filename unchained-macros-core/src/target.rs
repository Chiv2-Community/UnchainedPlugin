//! Capabilities of a verified native method, independent of its address source.

use crate::{
    AddressResolver, Callback, CallbackSlot, CallbackTarget, FunctionBinding, NativeHook, SupportsEntry, SupportsExit,
};

pub trait SupportsDetour: CallbackTarget {}

pub const fn detour_signature<T: SupportsDetour>(_: &T) -> crate::NativeSignature<T::Callback> {
    std::marker::PhantomData
}

/// One dispatcher's exact replacement, entry and exit signatures.
/// Function pointers stay typed all the way to dispatch, including higher-ranked lifetimes.
pub struct NativeTarget<F, E, X, R: AddressResolver> {
    binding: &'static FunctionBinding<R>,
    hook: NativeHook,
    entry_dispatch: F,
    exit_dispatch: F,
    code_pointer: fn(F) -> *const (),
    slot: CallbackSlot<Callback<F, E, X>>,
}

impl<F: Copy, E: Copy, X: Copy, R: AddressResolver> NativeTarget<F, E, X, R> {
    /// # Safety
    /// `binding` and `dispatcher` must honor F/E/X and original-call bypass.
    /// `code_pointer` must convert its argument to the unchanged code pointer.
    pub const unsafe fn new(
        binding: &'static FunctionBinding<R>,
        entry_dispatch: F,
        exit_dispatch: F,
        code_pointer: fn(F) -> *const (),
    ) -> Self {
        Self {
            binding,
            hook: NativeHook::new(),
            entry_dispatch,
            exit_dispatch,
            code_pointer,
            slot: CallbackSlot::new(),
        }
    }

    pub const fn binding(&self) -> &'static FunctionBinding<R> {
        self.binding
    }
}

impl<F, E, X, R: AddressResolver> CallbackTarget for NativeTarget<F, E, X, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    X: Copy + Send + Sync + 'static,
{
    type Callback = F;
    type Entry = E;
    type Exit = X;
    fn slot(&self) -> &CallbackSlot<Callback<F, E, X>> {
        &self.slot
    }
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo {
        self.binding.resolver
    }

    unsafe fn install_hook(&'static self, callback: Callback<F, E, X>) -> Result<(), Box<dyn std::error::Error>> {
        let dispatcher = match callback {
            Callback::Detour(replacement) => replacement,
            Callback::Entry(_) => self.entry_dispatch,
            Callback::Exit(_) => self.exit_dispatch,
        };
        unsafe { self.hook.attach(self.binding, (self.code_pointer)(dispatcher)) }
    }
}

impl<F, E, X, R: AddressResolver> SupportsEntry for NativeTarget<F, E, X, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    X: Copy + Send + Sync + 'static,
{
}

impl<F, E, X, R: AddressResolver> SupportsExit for NativeTarget<F, E, X, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    X: Copy + Send + Sync + 'static,
{
}

impl<F, E, X, R: AddressResolver> SupportsDetour for NativeTarget<F, E, X, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    X: Copy + Send + Sync + 'static,
{
}
