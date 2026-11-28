use crate::{schema::Parameter, KismetContext};
use std::{
    num::NonZeroUsize,
    sync::{Mutex, OnceLock},
};

pub(crate) fn current_thread_id() -> u32 {
    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() }
}

/// Resolver for a surviving, explicitly verified VM wrapper, never a C++ body.
pub struct KismetExecResolver {
    pub owner: &'static str,
    pub member: &'static str,
    arguments: &'static [Parameter],
    result: Option<Parameter>,
    address: OnceLock<NonZeroUsize>,
    properties: OnceLock<Box<[usize]>>,
    observation_thread: OnceLock<u32>,
    lock: Mutex<()>,
}

impl KismetExecResolver {
    /// # Safety
    /// The wrapper must step exactly these arguments, into live independent
    /// local slots lasting until its final Step returns. It must call the real
    /// original body and clean up its own temporaries. No ref/out redirection,
    /// skipped steps, custom parameter decoding, or inlined execution between
    /// arguments is supported. Reverify this contract when the binary changes.
    /// The wrapper must remain unique to this UFunction, including native
    /// functions not yet loaded at initialization.
    pub const unsafe fn new(
        owner: &'static str,
        member: &'static str,
        arguments: &'static [Parameter],
        result: Option<Parameter>,
    ) -> Self {
        Self {
            owner,
            member,
            arguments,
            result,
            address: OnceLock::new(),
            properties: OnceLock::new(),
            observation_thread: OnceLock::new(),
            lock: Mutex::new(()),
        }
    }

    pub fn properties(&'static self) -> &'static [usize] {
        self.properties
            .get()
            .expect("Exec schema must be published before attachment")
    }

    /// Foreign threads must forward the real wrapper without observing native
    /// text/history or invoking entry callbacks.
    pub fn is_observation_thread(&self) -> bool {
        self.observation_thread
            .get()
            .is_some_and(|thread| *thread == current_thread_id())
    }
}

impl unchained_macros_core::AddressResolver for KismetExecResolver {
    type Context = KismetContext;

    unsafe fn resolve(&self, context: &KismetContext) -> Result<NonZeroUsize, String> {
        let _guard = self.lock.lock().map_err(|_| "Exec resolver lock poisoned")?;

        if let Some(address) = self.address.get() {
            return Ok(*address);
        }

        let thread = context.observation_thread;

        if thread != current_thread_id() {
            return Err("Exec observation must resolve on the context's game thread".into());
        }

        let (function, schema) = unsafe { context.reflection.find_exec_schema(self.owner, self.member) }?;

        let properties = crate::schema::validate(&function, &schema, self.arguments, self.result)
            .map_err(|error| format!("{}::{}: {error}", self.owner, self.member))?;

        let address = NonZeroUsize::new(function.exec_thunk).ok_or("Exec wrapper address is null")?;

        let parameters = properties.len();

        self.properties
            .set(properties.into_boxed_slice())
            .map_err(|_| "Exec schema already published")?;

        self.observation_thread
            .set(thread)
            .map_err(|_| "Exec observation thread already published")?;

        self.address
            .set(address)
            .map_err(|_| "Exec address already published")?;

        log::info!(
            "Reflected exec {}::{}: thunk {:#x}, {} arguments",
            self.owner,
            self.member,
            address.get(),
            parameters
        );

        Ok(address)
    }
}

/// Observation target for a verified dummy exec wrapper. Only entry is supported.
pub struct DummyExecTarget<E> {
    binding: &'static unchained_macros_core::FunctionBinding<KismetExecResolver>,
    hook: unchained_macros_core::NativeHook,
    dispatcher: ExecDispatcher,
    pub entry: unchained_macros_core::CallbackSlot<
        unchained_macros_core::Callback<std::convert::Infallible, E, std::convert::Infallible>,
    >,
}

type ExecDispatcher = unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void);

impl<E: Copy> DummyExecTarget<E> {
    /// # Safety
    /// The VM dispatcher must forward the real wrapper and invoke E only with verified live step locals.
    pub const unsafe fn new(
        binding: &'static unchained_macros_core::FunctionBinding<KismetExecResolver>,
        dispatcher: ExecDispatcher,
    ) -> Self {
        Self {
            binding,
            hook: unchained_macros_core::NativeHook::new(),
            dispatcher,
            entry: unchained_macros_core::CallbackSlot::new(),
        }
    }
}

impl<E: Copy + Send + Sync + 'static> unchained_macros_core::CallbackTarget for DummyExecTarget<E> {
    type Callback = std::convert::Infallible;
    type Entry = E;
    type Exit = std::convert::Infallible;
    fn slot(
        &self,
    ) -> &unchained_macros_core::CallbackSlot<unchained_macros_core::Callback<Self::Callback, E, Self::Exit>> {
        &self.entry
    }
    fn resolver(&self) -> &'static dyn unchained_macros_core::__private::ResolverInfo {
        self.binding.resolver
    }

    unsafe fn install_hook(
        &'static self,
        _callback: unchained_macros_core::Callback<Self::Callback, E, Self::Exit>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { self.hook.attach(self.binding, self.dispatcher as *const ()) }
    }
}

impl<E: Copy + Send + Sync + 'static> unchained_macros_core::SupportsEntry for DummyExecTarget<E> {}
