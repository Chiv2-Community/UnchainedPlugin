//! Capabilities of a verified native method, independent of its address source.
use crate::{AddressResolver, CallbackSlot, CallbackTarget, FunctionBinding, NativeHook, SupportsEntry, SupportsExit};

pub trait SupportsDetour: CallbackTarget {
    type Callback: Copy + Send + Sync + 'static;
    fn replacement_slot(&self) -> &CallbackSlot<Self::Callback>;
    fn physical_detour(&self) -> Option<&retour::RawDetour>;

    /// # Safety
    /// Callers must be quiescent and the replacement must uphold the target's native contract.
    unsafe fn register_detour(
        &'static self,
        callback: Self::Callback,
        activate: bool,
        condition: fn() -> bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.replacement_slot().is_registered() {
            return Err("Only one behavior-changing detour is supported per method".into());
        }
        unsafe {
            self.ensure_attached()?;
        }
        unsafe {
            self.replacement_slot()
                .register(callback, self.activation(activate, condition), || true)?;
        }
        Ok(())
    }
}

/// One dispatcher's exact replacement, entry, exit and exit-preparation signatures.
/// Function pointers stay typed all the way to dispatch, including higher-ranked lifetimes.
pub struct NativeTarget<F, E, X, P, R: AddressResolver> {
    binding: &'static FunctionBinding<R>,
    hook: NativeHook,
    dispatcher: F,
    code_pointer: fn(F) -> *const (),
    replacement: CallbackSlot<F>,
    entry: CallbackSlot<E>,
    exit: CallbackSlot<P>,
    signature: std::marker::PhantomData<fn(X) -> X>,
}
impl<F: Copy, E: Copy, X, P: Copy, R: AddressResolver> NativeTarget<F, E, X, P, R> {
    /// # Safety
    /// `binding` and `dispatcher` must honor F/E/X/P and original-call bypass.
    /// `code_pointer` must convert its argument to the unchanged code pointer.
    pub const unsafe fn new(
        binding: &'static FunctionBinding<R>,
        dispatcher: F,
        code_pointer: fn(F) -> *const (),
    ) -> Self {
        Self {
            binding,
            hook: NativeHook::new(),
            dispatcher,
            code_pointer,
            replacement: CallbackSlot::new(),
            entry: CallbackSlot::new(),
            exit: CallbackSlot::new(),
            signature: std::marker::PhantomData,
        }
    }
    pub const fn binding(&self) -> &'static FunctionBinding<R> {
        self.binding
    }
}
impl<F, E, X: 'static, P, R: AddressResolver> CallbackTarget for NativeTarget<F, E, X, P, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    P: Copy + Send + Sync + 'static,
{
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo {
        self.binding.resolver
    }
    unsafe fn ensure_attached(&'static self) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { self.hook.attach(self.binding, (self.code_pointer)(self.dispatcher)) }
    }
}
impl<F, E, X: 'static, P, R: AddressResolver> SupportsEntry for NativeTarget<F, E, X, P, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    P: Copy + Send + Sync + 'static,
{
    type Entry = E;
    fn entry_slot(&self) -> &CallbackSlot<E> {
        &self.entry
    }
}
impl<F, E, X: Copy + Sync + 'static, P, R: AddressResolver> SupportsExit for NativeTarget<F, E, X, P, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    P: Copy + Send + Sync + 'static,
{
    type Exit = X;
    type Prepare = P;
    fn exit_slot(&self) -> &CallbackSlot<P> {
        &self.exit
    }
}
impl<F, E, X: 'static, P, R: AddressResolver> SupportsDetour for NativeTarget<F, E, X, P, R>
where
    F: Copy + Send + Sync + 'static,
    E: Copy + Send + Sync + 'static,
    P: Copy + Send + Sync + 'static,
{
    type Callback = F;
    fn replacement_slot(&self) -> &CallbackSlot<F> {
        &self.replacement
    }
    fn physical_detour(&self) -> Option<&retour::RawDetour> {
        self.hook.detour()
    }
}
