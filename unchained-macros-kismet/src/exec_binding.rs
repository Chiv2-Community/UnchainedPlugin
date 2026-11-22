use std::{num::NonZeroUsize, sync::{Mutex, OnceLock}};
use crate::{KismetContext, schema::Parameter};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObservationThread {
    #[cfg(windows)]
    id: u32,
    #[cfg(not(windows))]
    id: std::thread::ThreadId,
}
impl ObservationThread {
    pub(crate) fn current() -> Option<Self> {
        #[cfg(windows)]
        { Some(Self { id: unsafe { windows::Win32::System::Threading::GetCurrentThreadId() } }) }
        #[cfg(not(windows))]
        { unchained_macros_core::isolate_observer(|| Self { id: std::thread::current().id() }) }
    }
}

/// Resolver for a surviving, explicitly verified VM wrapper, never a C++ body.
pub struct KismetExecResolver {
    pub owner: &'static str,
    pub member: &'static str,
    arguments: &'static [Parameter],
    result: Option<Parameter>,
    address: OnceLock<NonZeroUsize>,
    properties: OnceLock<Box<[usize]>>,
    observation_thread: OnceLock<ObservationThread>,
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
    /// functions not yet loaded at initialization; reflected aliases are rejected.
    pub const unsafe fn new(
        owner: &'static str, member: &'static str,
        arguments: &'static [Parameter], result: Option<Parameter>,
    ) -> Self {
        Self { owner, member, arguments, result, address: OnceLock::new(),
            properties: OnceLock::new(), observation_thread: OnceLock::new(), lock: Mutex::new(()) }
    }

    pub fn properties(&'static self) -> &'static [usize] {
        self.properties.get().expect("Exec schema must be published before attachment")
    }

    /// Foreign threads must forward the real wrapper without observing native
    /// text/history or invoking entry callbacks.
    pub fn is_observation_thread(&self) -> bool {
        self.observation_thread.get().is_some_and(|thread| Some(*thread) == ObservationThread::current())
    }
}

impl unchained_macros_core::AddressResolver for KismetExecResolver {
    type Context = KismetContext;
    unsafe fn resolve(&self, context: &KismetContext) -> Result<NonZeroUsize, String> {
        let _guard = self.lock.lock().map_err(|_| "Exec resolver lock poisoned")?;
        if let Some(address) = self.address.get() { return Ok(*address); }
        let thread = context.observation_thread
            .filter(|thread| Some(*thread) == ObservationThread::current())
            .ok_or("Exec observation must resolve on the context's game thread")?;
        let function = unsafe { context.reflection.find_function(self.owner, self.member) }?;
        let schema = unsafe { context.reflection.find_exec_schema(self.owner, self.member) }?;
        let properties = crate::schema::validate(&function, &schema, self.arguments, self.result)
            .map_err(|error| format!("{}::{}: {error}", self.owner, self.member))?;
        let address = NonZeroUsize::new(function.exec_thunk).ok_or("Exec wrapper address is null")?;
        let parameters = properties.len();
        self.properties.set(properties.into_boxed_slice()).map_err(|_| "Exec schema already published")?;
        self.observation_thread.set(thread).map_err(|_| "Exec observation thread already published")?;
        self.address.set(address).map_err(|_| "Exec address already published")?;
        log::info!("Reflected exec {}::{}: thunk {:#x}, {} arguments",
            self.owner, self.member, address.get(), parameters);
        Ok(address)
    }
}

/// Exec-only methods deliberately provide no native-call/detour descriptor.
/// This includes shared no-op and cleanup-only bodies; never patch those bodies.
pub struct DummyExecMethodCannotBeDetoured;

/// Dummy exec methods support entry observation only.
pub enum DummyExecMethodHasNoExitListener {}
