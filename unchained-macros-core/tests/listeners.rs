use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

use unchained_macros_core::*;

struct ObservationTarget<E, X, P, R: AddressResolver> {
    pub resolver: &'static Resolver<R>,
    pub entry: CallbackSlot<E>,
    pub exit: CallbackSlot<P>,
    attach: unsafe fn() -> Result<(), Box<dyn std::error::Error>>,
    signature: std::marker::PhantomData<fn(X) -> X>,
}

impl<E: Copy, X, P: Copy, R: AddressResolver> ObservationTarget<E, X, P, R> {
    /// # Safety
    /// Attachment and dispatch must honor E/X/P, preserve original execution,
    /// and finish snapshots only with the declared caller-visible result.
    pub const unsafe fn new(
        resolver: &'static Resolver<R>,
        attach: unsafe fn() -> Result<(), Box<dyn std::error::Error>>,
    ) -> Self {
        Self {
            resolver,
            entry: CallbackSlot::new(),
            exit: CallbackSlot::new(),
            attach,
            signature: std::marker::PhantomData,
        }
    }
}

impl<E: Copy + Send + Sync + 'static, X: 'static, P: Copy + Send + Sync + 'static, R: AddressResolver> CallbackTarget
    for ObservationTarget<E, X, P, R>
{
    fn resolver(&self) -> &'static dyn unchained_macros_core::__private::ResolverInfo {
        self.resolver
    }

    unsafe fn ensure_attached(&'static self) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { (self.attach)() }
    }
}

impl<E: Copy + Send + Sync + 'static, X: 'static, P: Copy + Send + Sync + 'static, R: AddressResolver> SupportsEntry
    for ObservationTarget<E, X, P, R>
{
    type Entry = E;
    fn entry_slot(&self) -> &CallbackSlot<E> {
        &self.entry
    }
}

impl<
        E: Copy + Send + Sync + 'static,
        X: Copy + Sync + 'static,
        P: Copy + Send + Sync + 'static,
        R: AddressResolver,
    > SupportsExit for ObservationTarget<E, X, P, R>
{
    type Exit = X;
    type Prepare = P;
    fn exit_slot(&self) -> &CallbackSlot<P> {
        &self.exit
    }
}

struct Backend;
impl AddressResolver for Backend {
    type Context = ();
    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        unreachable!()
    }
}

static RESOLVER: Resolver<Backend> = Resolver {
    name: "fixture",
    implementation: Backend,
    filter: None,
};

struct Target;
impl ResolverDeclaration for Target {
    type Backend = Backend;
    const NAME: &'static str = "fixture";
    const UNRESTRICTED: bool = true;
    const DESCRIPTOR: &'static Resolver<Backend> = &RESOLVER;
}

struct Native;
#[runtime_bindings]
impl Native {
    #[bind_resolved_func(Target)]
    unsafe fn call(value: i32, output: *mut i32) -> i32 {}
    #[bind_resolved_func(Backend, &RESOLVER)]
    unsafe fn descriptor_alias(value: i32, output: *mut i32) -> i32 {}
}

static EVENTS: Mutex<Vec<(char, i32)>> = Mutex::new(Vec::new());
static ORIGINAL_CALLS: AtomicUsize = AtomicUsize::new(0);
static DETOUR_CONDITION: AtomicBool = AtomicBool::new(true);
static ENTRY_CONDITION: AtomicBool = AtomicBool::new(true);
static DETOUR_CONDITION_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTRY_CONDITION_CALLS: AtomicUsize = AtomicUsize::new(0);
static SKIP_ORIGINAL: AtomicBool = AtomicBool::new(false);
static NEST: AtomicBool = AtomicBool::new(false);
static PANIC_ENTRY: AtomicBool = AtomicBool::new(false);
static REENTER_ENTRY: AtomicBool = AtomicBool::new(false);
static REENTER_EXIT: AtomicBool = AtomicBool::new(false);
static ADJUST_INPUT: AtomicBool = AtomicBool::new(false);

#[inline(never)]
unsafe extern "C" fn original(value: i32, output: *mut i32) -> i32 {
    ORIGINAL_CALLS.fetch_add(1, Ordering::SeqCst);
    EVENTS.lock().unwrap().push(('o', value));
    unsafe {
        *output += value;
    }

    value + 1
}

#[bind_func_entry_listener(Native::call, || {
    ENTRY_CONDITION_CALLS.fetch_add(1, Ordering::SeqCst);
    ENTRY_CONDITION.load(Ordering::SeqCst)
})]

unsafe fn native_entry(value: &i32, _output: &*mut i32) {
    EVENTS.lock().unwrap().push(('e', *value));
    if PANIC_ENTRY.load(Ordering::SeqCst) {
        panic!("entry failed");
    }

    if REENTER_ENTRY.load(Ordering::SeqCst) {
        let mut output = 0;
        unsafe {
            original(*value + 100, &mut output);
        }
    }
}

#[bind_func_exit_listener(Native::call)]
unsafe fn native_exit(value: &i32, output: &*mut i32, result: &i32) {
    EVENTS.lock().unwrap().push(('x', *value));
    EVENTS.lock().unwrap().push(('r', *result));
    // Raw pointers snapshot addresses, not pointee values.
    assert_eq!(
        unsafe { **output },
        if SKIP_ORIGINAL.load(Ordering::SeqCst) {
            0
        } else {
            *value + if ADJUST_INPUT.load(Ordering::SeqCst) { 10 } else { 0 }
        }
    );
    if REENTER_EXIT.load(Ordering::SeqCst) {
        let mut output = 0;
        unsafe {
            original(*value + 200, &mut output);
        }
    }
}

#[bind_func_detour(Native::call, || {
    DETOUR_CONDITION_CALLS.fetch_add(1, Ordering::SeqCst);
    DETOUR_CONDITION.load(Ordering::SeqCst)
})]

unsafe fn coexistence_detour(value: i32, output: *mut i32) -> i32 {
    EVENTS.lock().unwrap().push(('d', value));
    if NEST.load(Ordering::SeqCst) && value > 1 {
        let mut nested_output = 0;
        unsafe {
            original(value - 1, &mut nested_output);
        }
    }

    if SKIP_ORIGINAL.load(Ordering::SeqCst) {
        99
    } else {
        let input = value + if ADJUST_INPUT.load(Ordering::SeqCst) { 10 } else { 0 };
        unsafe { Native::call(input, output) + 10 }
    }
}

#[bind_func_entry_listener(Native::call)]
unsafe fn duplicate_entry(_value: &i32, _output: &*mut i32) {}
#[bind_func_exit_listener(Native::call)]
unsafe fn duplicate_exit(_value: &i32, _output: &*mut i32, _result: &i32) {}
#[bind_func_detour(Native::call)]
unsafe fn duplicate_detour(value: i32, output: *mut i32) -> i32 {
    unsafe { Native::call(value, output) }
}

#[test]
fn native_coexistence_bypass_conditions_and_aliases() {
    let binding = Native::__UNCHAINED_TARGET_call.binding();
    let listeners = listener_inventory_for::<Backend>()
        .filter(|listener| !listener.name.starts_with("duplicate_"))
        .collect::<Vec<_>>();
    // Failed unresolved attachment does not publish slots; retry after resolution works.
    for registration in &listeners {
        assert!(unsafe { registration.attach(true) }
            .unwrap_err()
            .to_string()
            .contains("not initialized"));
        assert!(!registration.initialized());
    }

    unsafe {
        binding
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();
    }

    // Register in reverse dispatch order; registration order is immaterial.
    for registration in &listeners {
        unsafe {
            registration.attach(true).unwrap();
        }
    }

    assert!(listener_is_initialized("native_entry"));
    assert!(listener_is_enabled("native_entry"));
    let hook = detour_inventory_for::<Backend>()
        .find(|hook| hook.name() == "coexistence_detour")
        .unwrap();
    unsafe {
        hook.attach(true).unwrap();
    }

    assert!(detour_is_enabled("coexistence_detour"));
    let mut output = 0;
    assert_eq!(unsafe { original(4, &mut output) }, 15);
    assert_eq!(output, 4);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [('e', 4), ('d', 4), ('o', 4), ('x', 4), ('r', 15)]
    );
    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), 1);
    EVENTS.lock().unwrap().clear();

    // Project-native calls bypass every observer and the behavior-changing detour.
    output = 0;
    assert_eq!(unsafe { Native::call(5, &mut output) }, 6);
    assert_eq!(*EVENTS.lock().unwrap(), [('o', 5)]);
    EVENTS.lock().unwrap().clear();
    // Descriptor bindings inherit the same trampoline, including late aliases.
    let alias = Native::__UNCHAINED_TARGET_descriptor_alias.binding();
    unsafe {
        alias
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();
    }

    assert_eq!(alias.address(), binding.address());
    assert_eq!(unsafe { Native::descriptor_alias(5, &mut output) }, 6);
    assert_eq!(*EVENTS.lock().unwrap(), [('o', 5)]);
    EVENTS.lock().unwrap().clear();

    // Conditions are frozen at attachment, not reevaluated by calls/status.
    DETOUR_CONDITION.store(false, Ordering::SeqCst);
    ENTRY_CONDITION.store(false, Ordering::SeqCst);
    assert!(detour_is_enabled("coexistence_detour"));
    output = 0;
    assert_eq!(unsafe { original(6, &mut output) }, 17);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [('e', 6), ('d', 6), ('o', 6), ('x', 6), ('r', 17)]
    );
    EVENTS.lock().unwrap().clear();
    hook.set_enabled(false);
    Native::__UNCHAINED_TARGET_call.entry_slot().set_enabled(false);
    assert!(!listener_is_enabled("native_entry"));
    assert!(listener_is_enabled("native_exit"));
    assert!(!detour_is_enabled("coexistence_detour"));
    output = 0;
    assert_eq!(unsafe { original(6, &mut output) }, 7);
    assert_eq!(*EVENTS.lock().unwrap(), [('o', 6), ('x', 6), ('r', 7)]);
    EVENTS.lock().unwrap().clear();
    DETOUR_CONDITION.store(true, Ordering::SeqCst);
    ENTRY_CONDITION.store(true, Ordering::SeqCst);
    Native::__UNCHAINED_TARGET_call.entry_slot().set_enabled(true);
    hook.set_enabled(false);
    assert!(!detour_is_enabled("coexistence_detour"));
    output = 0;
    assert_eq!(unsafe { original(7, &mut output) }, 8);
    assert_eq!(*EVENTS.lock().unwrap(), [('e', 7), ('o', 7), ('x', 7), ('r', 8)]);
    EVENTS.lock().unwrap().clear();
    hook.set_enabled(true);

    SKIP_ORIGINAL.store(true, Ordering::SeqCst);
    output = 0;
    assert_eq!(unsafe { original(8, &mut output) }, 99);
    assert_eq!(output, 0);
    assert_eq!(*EVENTS.lock().unwrap(), [('e', 8), ('d', 8), ('x', 8), ('r', 99)]);
    EVENTS.lock().unwrap().clear();

    let before = ORIGINAL_CALLS.load(Ordering::SeqCst);
    let alias: &'static FunctionBinding<Backend> = Box::leak(Box::new(FunctionBinding::new("late alias", &RESOLVER)));
    unsafe {
        alias
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();
    }

    assert_eq!(alias.address(), binding.address());
    let bypass: unsafe extern "C" fn(i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(alias.address().unwrap().get()) };
    assert_eq!(unsafe { bypass(9, &mut output) }, 10);
    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), before + 1);
    EVENTS.lock().unwrap().clear();
    SKIP_ORIGINAL.store(false, Ordering::SeqCst);
    NEST.store(true, Ordering::SeqCst);
    output = 0;
    assert_eq!(unsafe { original(2, &mut output) }, 13);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [
            ('e', 2),
            ('d', 2),
            ('e', 1),
            ('d', 1),
            ('o', 1),
            ('x', 1),
            ('r', 12),
            ('o', 2),
            ('x', 2),
            ('r', 13),
        ]
    );
    EVENTS.lock().unwrap().clear();
    NEST.store(false, Ordering::SeqCst);
    PANIC_ENTRY.store(true, Ordering::SeqCst);
    output = 0;
    assert_eq!(unsafe { original(3, &mut output) }, 14);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [('e', 3), ('d', 3), ('o', 3), ('x', 3), ('r', 14)]
    );
    EVENTS.lock().unwrap().clear();
    PANIC_ENTRY.store(false, Ordering::SeqCst);
    REENTER_ENTRY.store(true, Ordering::SeqCst);
    REENTER_EXIT.store(true, Ordering::SeqCst);
    output = 0;
    assert_eq!(unsafe { original(3, &mut output) }, 14);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [
            ('e', 3),
            ('d', 103),
            ('o', 103),
            ('d', 3),
            ('o', 3),
            ('x', 3),
            ('r', 14),
            ('d', 203),
            ('o', 203),
        ]
    );
    EVENTS.lock().unwrap().clear();
    REENTER_ENTRY.store(false, Ordering::SeqCst);
    REENTER_EXIT.store(false, Ordering::SeqCst);
    ADJUST_INPUT.store(true, Ordering::SeqCst);
    output = 0;
    assert_eq!(unsafe { original(3, &mut output) }, 24);
    assert_eq!(output, 13);
    assert_eq!(
        *EVENTS.lock().unwrap(),
        [('e', 3), ('d', 3), ('o', 13), ('x', 3), ('r', 24)]
    );
    assert_eq!(DETOUR_CONDITION_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(ENTRY_CONDITION_CALLS.load(Ordering::SeqCst), 1);

    for registration in listeners {
        assert!(unsafe { registration.attach(true) }.is_err());
    }

    assert!(unsafe { hook.attach(true) }.is_err());
    for registration in listener_inventory_for::<Backend>().filter(|listener| listener.name.starts_with("duplicate_")) {
        assert!(unsafe { registration.attach(true) }
            .unwrap_err()
            .to_string()
            .contains("Only one"));
    }

    let duplicate = detour_inventory_for::<Backend>()
        .find(|hook| hook.name() == "duplicate_detour")
        .unwrap();
    assert!(unsafe { duplicate.attach(true) }
        .unwrap_err()
        .to_string()
        .contains("Only one behavior-changing"));
    assert!(duplicate.detour().is_none());
    duplicate.set_enabled(false);
    assert!(hook.enabled());
    hook.set_enabled(false);
    duplicate.set_enabled(true);
    assert!(!hook.enabled());
    assert!(!duplicate.enabled());
    hook.set_enabled(true);

    struct OtherBackend;
    impl AddressResolver for OtherBackend {
        type Context = ();
        unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
            unreachable!()
        }
    }

    static OTHER_RESOLVER: Resolver<OtherBackend> = Resolver {
        name: "cross-backend alias",
        implementation: OtherBackend,
        filter: None,
    };

    let other = Box::leak(Box::new(FunctionBinding::new("other backend", &OTHER_RESOLVER)));
    unsafe {
        other
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();
    }

    assert_eq!(other.address(), binding.address());
    let physical = Box::leak(Box::new(NativeHook::new()));
    assert!(unsafe { physical.attach(other, original as *const ()) }
        .unwrap_err()
        .to_string()
        .contains("already owns native address"));
    assert!(physical.detour().is_none());
}

#[test]
fn callback_panics_and_dropped_snapshots_are_isolated() {
    assert!(isolate_observer(|| panic!("observer failure")).is_none());
    let slot = CallbackSlot::new();
    unsafe {
        slot.register(12_u32, true, || panic!("condition failure")).unwrap();
    }

    assert!(slot.get().is_none());
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    unsafe fn finish(_: *mut (), _: *const ()) {
        panic!("exit failure");
    }

    struct Snapshot;
    impl Drop for Snapshot {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    let make = || unsafe { ExitObservation::from_snapshot(Snapshot, finish) };
    drop(make());
    unsafe {
        make().finish(std::ptr::null());
    }

    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}

#[test]
fn descriptor_guard_is_thread_local_and_restored_after_panics() {
    type Descriptor = ObservationTarget<unsafe fn(&u32), (), (), Backend>;
    unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    unsafe fn callback(_: &u32) {}
    static FIRST: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
    static SECOND: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
    unsafe {
        FIRST.register_entry(callback, true, || true).unwrap();
        SECOND.register_entry(callback, true, || true).unwrap();
    }

    FIRST.observe_entry(|_| {
        assert!(FIRST.observe_entry(|_| ()).is_none());
        assert!(SECOND.observe_entry(|_| ()).is_some());
        std::thread::spawn(|| assert!(FIRST.observe_entry(|_| ()).is_some()))
            .join()
            .unwrap();
        panic!("restore on failure");
    });

    assert!(FIRST.observe_entry(|_| ()).is_some());
}

macro_rules! fixture_resolver {
    () => {
        struct Backend;
        impl AddressResolver for Backend {
            type Context = ();
            unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
                unreachable!()
            }
        }

        static RESOLVER: Resolver<Backend> = Resolver {
            name: module_path!(),
            implementation: Backend,
            filter: None,
        };

        #[allow(dead_code)]
        struct Target;
        impl ResolverDeclaration for Target {
            type Backend = Backend;
            const NAME: &'static str = module_path!();
            const UNRESTRICTED: bool = true;
            const DESCRIPTOR: &'static Resolver<Backend> = &RESOLVER;
        }
    };
}

mod backend_specific {
    use super::*;
    fixture_resolver!();
    static ATTACHED: AtomicBool = AtomicBool::new(false);
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    type Descriptor =
        ObservationTarget<unsafe fn(&u32), unsafe fn(&u32, &bool), unsafe fn(&u32) -> ExitObservation, Backend>;
    unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> {
        ATTACHED.store(true, Ordering::SeqCst);
        Ok(())
    }

    static DESCRIPTOR: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
    struct VmOnly;
    impl VmOnly {
        #[allow(non_upper_case_globals)]
        const __UNCHAINED_TARGET_call: &'static Descriptor = &DESCRIPTOR;
    }

    #[bind_func_entry_listener(VmOnly::call)]
    unsafe fn entry(value: &u32) {
        CALLS.fetch_add(*value as usize, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(VmOnly::call)]
    unsafe fn exit(value: &u32, result: &bool) {
        assert!(*result);
        CALLS.fetch_add(*value as usize, Ordering::SeqCst);
    }

    #[test]
    fn observation_only_target_does_not_need_native_function_or_callable_method() {
        validate_listener_registrations_for::<Backend>(&SelectorSnapshot::default()).unwrap();
        for listener in listener_inventory_for::<Backend>() {
            unsafe {
                listener.attach(true).unwrap();
            }
        }

        assert!(ATTACHED.load(Ordering::SeqCst));
        assert_eq!(binding_inventory_for::<Backend>().count(), 0);
        assert_eq!(detour_inventory_for::<Backend>().count(), 0);
        DESCRIPTOR.observe_entry(|callback| unsafe { callback(&3) });
        let observation = DESCRIPTOR.prepare_exit(|prepare| unsafe { prepare(&3) }).unwrap();
        // Backend execution remains independent of core's native ABI machinery.
        let result = true;
        unsafe {
            DESCRIPTOR.finish_exit(observation, (&result as *const bool).cast());
        }

        assert_eq!(CALLS.load(Ordering::SeqCst), 6);
    }
}

mod duplicate_discovery {
    use super::*;
    fixture_resolver!();
    static ATTACHED: AtomicBool = AtomicBool::new(false);
    static ATTACHMENTS: AtomicUsize = AtomicUsize::new(0);
    type Descriptor =
        ObservationTarget<unsafe fn(&u32), unsafe fn(&u32, &u32), unsafe fn(&u32) -> ExitObservation, Backend>;
    unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> {
        if !ATTACHED.swap(true, Ordering::SeqCst) {
            ATTACHMENTS.fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    }

    static DESCRIPTOR: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
    struct VmOnly;
    impl VmOnly {
        #[allow(non_upper_case_globals)]
        const __UNCHAINED_TARGET_call: &'static Descriptor = &DESCRIPTOR;
    }

    // Identical bodies may be linker-folded; identity must come from slot storage.
    #[bind_func_entry_listener(VmOnly::call)]
    unsafe fn first_entry(_value: &u32) {}
    #[bind_func_entry_listener(VmOnly::call)]
    unsafe fn second_entry(_value: &u32) {}
    #[bind_func_exit_listener(VmOnly::call)]
    unsafe fn first_exit(_value: &u32, _result: &u32) {}
    #[bind_func_exit_listener(VmOnly::call)]
    unsafe fn second_exit(_value: &u32, _result: &u32) {}

    unsafe fn host_loop(snapshot: &SelectorSnapshot, preflight: bool) -> Result<(), Box<dyn std::error::Error>> {
        if preflight {
            validate_listener_registrations_for::<Backend>(snapshot)?;
        }

        for listener in listener_inventory_for::<Backend>() {
            if !listener.resolver().supports(snapshot) || listener.initialized() {
                continue;
            }

            unsafe {
                listener.attach(true)?;
            }
        }

        Ok(())
    }

    #[test]
    fn host_discovery_rejects_duplicates_before_attachment_and_never_steals_status() {
        let snapshot = SelectorSnapshot::default();
        let error = unsafe { host_loop(&snapshot, true) }.unwrap_err().to_string();
        assert!(error.contains("Only one listener per method phase"), "{error}");
        assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 0);
        for listener in listener_inventory_for::<Backend>() {
            assert!(!listener.initialized());
            assert!(!listener.enabled());
        }

        // Even a host without preflight must not skip a loser as "initialized".
        for listener in listener_inventory_for::<Backend>().filter(|listener| listener.name.starts_with("first_")) {
            unsafe {
                listener.attach(true).unwrap();
            }
        }

        assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 1);
        let error = unsafe { host_loop(&snapshot, false) }.unwrap_err().to_string();
        assert!(error.contains("Only one"), "{error}");
        for listener in listener_inventory_for::<Backend>() {
            if listener.name.starts_with("second_") {
                assert!(!listener.initialized());
                assert!(!listener.enabled());
                listener.set_enabled(false);
            }
        }

        for listener in listener_inventory_for::<Backend>().filter(|listener| listener.name.starts_with("first_")) {
            assert!(listener.initialized());
            assert!(listener.enabled());
        }

        assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 1);
    }
}

mod activation {
    use super::*;
    fixture_resolver!();
    static CONDITIONS: AtomicUsize = AtomicUsize::new(0);
    static ENTRIES: AtomicUsize = AtomicUsize::new(0);
    fn disabled() -> bool {
        CONDITIONS.fetch_add(1, Ordering::SeqCst);
        false
    }

    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Backend, &RESOLVER)]
        unsafe fn off() -> usize {}
        #[bind_resolved_func(Target)]
        unsafe fn gated() -> usize {}
    }

    #[inline(never)]
    unsafe extern "C" fn original_off() -> usize {
        std::hint::black_box(11)
    }

    #[inline(never)]
    unsafe extern "C" fn original_gated() -> usize {
        std::hint::black_box(12)
    }

    #[bind_func_detour(Native::off, disabled)]
    unsafe fn off() -> usize {
        99
    }

    #[bind_func_detour(Native::gated, disabled)]
    unsafe fn gated() -> usize {
        98
    }

    #[bind_func_entry_listener(Native::off)]
    unsafe fn entry() {
        ENTRIES.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn global_autoactivation_off_skips_condition_and_conditions_are_not_dispatch_predicates() {
        unsafe {
            Native::__UNCHAINED_TARGET_off
                .binding()
                .initialize_address(NonZeroUsize::new(original_off as *const () as usize).unwrap())
                .unwrap();
            Native::__UNCHAINED_TARGET_gated
                .binding()
                .initialize_address(NonZeroUsize::new(original_gated as *const () as usize).unwrap())
                .unwrap();
            let off = detour_inventory_for::<Backend>()
                .find(|hook| hook.name() == "off")
                .unwrap();
            let gated = detour_inventory_for::<Backend>()
                .find(|hook| hook.name() == "gated")
                .unwrap();
            off.attach(false).unwrap();
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 0);
            gated.attach(true).unwrap();
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 1);
            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            assert!(!off.enabled());
            assert!(!gated.enabled());
            assert_eq!(original_off(), 11);
            assert_eq!(original_gated(), 12);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
            off.set_enabled(true);
            assert!(off.enabled());
            assert_eq!(original_off(), 99);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 2);
            assert_eq!(Native::off(), 11);
            assert_eq!(Native::gated(), 12);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 2);
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 1);
        }
    }
}

mod condition_reentry {
    use super::*;
    fixture_resolver!();
    static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static ENTRIES: AtomicUsize = AtomicUsize::new(0);
    static EXITS: AtomicUsize = AtomicUsize::new(0);
    static CONDITIONS: AtomicUsize = AtomicUsize::new(0);
    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Target)]
        unsafe fn call(value: usize) -> usize {}
    }

    #[inline(never)]
    unsafe extern "C" fn original(value: usize) -> usize {
        ORIGINAL.fetch_add(1, Ordering::SeqCst);
        value + 1
    }

    fn condition() -> bool {
        CONDITIONS.fetch_add(1, Ordering::SeqCst);
        assert_eq!(unsafe { original(5) }, 6);
        true
    }

    #[bind_func_entry_listener(Native::call)]
    unsafe fn entry(_value: &usize) {
        ENTRIES.fetch_add(1, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(Native::call, condition)]
    unsafe fn exit(_value: &usize, _result: &usize) {
        EXITS.fetch_add(1, Ordering::SeqCst);
    }

    #[bind_func_detour(Native::call, condition)]
    unsafe fn detour(value: usize) -> usize {
        unsafe { Native::call(value) + 10 }
    }

    #[test]
    fn exit_and_detour_attachment_conditions_suppress_existing_observers_not_execution() {
        unsafe {
            Native::__UNCHAINED_TARGET_call
                .binding()
                .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
                .unwrap();
            let entry = listener_inventory_for::<Backend>()
                .find(|listener| listener.name == "entry")
                .unwrap();
            let exit = listener_inventory_for::<Backend>()
                .find(|listener| listener.name == "exit")
                .unwrap();
            entry.attach(true).unwrap();
            exit.attach(true).unwrap();
            let detour = detour_inventory_for::<Backend>().next().unwrap();
            detour.attach(true).unwrap();
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 2);
            assert_eq!(ORIGINAL.load(Ordering::SeqCst), 2);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 0);
            assert_eq!(EXITS.load(Ordering::SeqCst), 0);
            assert_eq!(original(7), 18);
            assert_eq!(ORIGINAL.load(Ordering::SeqCst), 3);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
            assert_eq!(EXITS.load(Ordering::SeqCst), 1);
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 2);
        }
    }
}

mod local_hygiene {
    use super::*;
    fixture_resolver!();
    static ENTRY: AtomicUsize = AtomicUsize::new(0);
    static EXIT: AtomicUsize = AtomicUsize::new(0);
    static OBSERVATIONS: AtomicUsize = AtomicUsize::new(0);
    fn checksum(values: [usize; 12]) -> usize {
        values.into_iter().enumerate().fold(0_usize, |sum, (index, value)| {
            sum.wrapping_add(value.wrapping_mul(index + 1))
        })
    }

    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Target)]
        unsafe fn call(
            callback: *mut usize,
            address: *const usize,
            result: usize,
            snapshot: usize,
            __unchained_callback: usize,
            __unchained_address: usize,
            __unchained_result: usize,
            __unchained_snapshot: usize,
            __unchained_observation: usize,
            __unchained_prepare: usize,
            __unchained_call: usize,
            __unchained_native_call: usize,
        ) -> usize {
        }
    }

    #[inline(never)]
    unsafe extern "C" fn original(
        callback: *mut usize,
        address: *const usize,
        result: usize,
        snapshot: usize,
        __unchained_callback: usize,
        __unchained_address: usize,
        __unchained_result: usize,
        __unchained_snapshot: usize,
        __unchained_observation: usize,
        __unchained_prepare: usize,
        __unchained_call: usize,
        __unchained_native_call: usize,
    ) -> usize {
        checksum([
            callback as usize,
            address as usize,
            result,
            snapshot,
            __unchained_callback,
            __unchained_address,
            __unchained_result,
            __unchained_snapshot,
            __unchained_observation,
            __unchained_prepare,
            __unchained_call,
            __unchained_native_call,
        ])
    }

    #[bind_func_entry_listener(Native::call)]
    unsafe fn entry(
        callback: &*mut usize,
        address: &*const usize,
        result: &usize,
        snapshot: &usize,
        __unchained_callback: &usize,
        __unchained_address: &usize,
        __unchained_result: &usize,
        __unchained_snapshot: &usize,
        __unchained_observation: &usize,
        __unchained_prepare: &usize,
        __unchained_call: &usize,
        __unchained_native_call: &usize,
    ) {
        ENTRY.store(
            checksum([
                *callback as usize,
                *address as usize,
                *result,
                *snapshot,
                *__unchained_callback,
                *__unchained_address,
                *__unchained_result,
                *__unchained_snapshot,
                *__unchained_observation,
                *__unchained_prepare,
                *__unchained_call,
                *__unchained_native_call,
            ]),
            Ordering::SeqCst,
        );
        OBSERVATIONS.fetch_add(1, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(Native::call)]
    unsafe fn __unchained_snapshot(
        callback: &*mut usize,
        address: &*const usize,
        result: &usize,
        snapshot: &usize,
        __unchained_callback: &usize,
        __unchained_address: &usize,
        __unchained_result: &usize,
        __unchained_snapshot: &usize,
        __unchained_observation: &usize,
        __unchained_prepare: &usize,
        __unchained_call: &usize,
        __unchained_native_call: &usize,
        return_value: &usize,
    ) {
        let inputs = checksum([
            *callback as usize,
            *address as usize,
            *result,
            *snapshot,
            *__unchained_callback,
            *__unchained_address,
            *__unchained_result,
            *__unchained_snapshot,
            *__unchained_observation,
            *__unchained_prepare,
            *__unchained_call,
            *__unchained_native_call,
        ]);
        assert_eq!(inputs, *return_value);
        EXIT.store(inputs, Ordering::SeqCst);
        OBSERVATIONS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn user_parameter_names_do_not_alias_dispatcher_snapshot_or_original_wrapper_locals() {
        unsafe {
            Native::__UNCHAINED_TARGET_call
                .binding()
                .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
                .unwrap();
            validate_listener_registrations_for::<Backend>(&SelectorSnapshot::default()).unwrap();
            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            let mut callback_value = 0_usize;
            let address_value = 0_usize;
            let callback = &mut callback_value as *mut usize;
            let address = &address_value as *const usize;
            let expected = checksum([callback as usize, address as usize, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
            assert_eq!(original(callback, address, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12), expected);
            assert_eq!(ENTRY.load(Ordering::SeqCst), expected);
            assert_eq!(EXIT.load(Ordering::SeqCst), expected);
            assert_eq!(OBSERVATIONS.load(Ordering::SeqCst), 2);
            assert_eq!(
                Native::call(callback, address, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12),
                expected
            );
            assert_eq!(OBSERVATIONS.load(Ordering::SeqCst), 2);
        }
    }
}

mod ownership {
    use super::*;
    fixture_resolver!();
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    static SEEN: AtomicUsize = AtomicUsize::new(0);
    #[repr(C)]
    struct Owned {
        value: usize,
        padding: [usize; 3],
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Target)]
        unsafe fn echo(value: Owned) -> Owned {}
        #[bind_resolved_func(Target)]
        unsafe fn make(value: usize) -> Owned {}
    }

    #[inline(never)]
    unsafe extern "C" fn original_echo(value: Owned) -> Owned {
        value
    }

    #[inline(never)]
    unsafe extern "C" fn original_make(value: usize) -> Owned {
        Owned {
            value,
            padding: [1, 2, 3],
        }
    }

    #[bind_func_entry_listener(Native::echo)]
    unsafe fn entry(value: &Owned) {
        SEEN.store(value.value, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(Native::make)]
    unsafe fn exit(value: &usize, result: &Owned) {
        assert_eq!(*value, result.value);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
        SEEN.store(result.value, Ordering::SeqCst);
    }

    #[test]
    fn owned_input_and_aggregate_return_are_borrowed_without_extra_drops() {
        unsafe {
            Native::__UNCHAINED_TARGET_echo
                .binding()
                .initialize_address(NonZeroUsize::new(original_echo as *const () as usize).unwrap())
                .unwrap();
            Native::__UNCHAINED_TARGET_make
                .binding()
                .initialize_address(NonZeroUsize::new(original_make as *const () as usize).unwrap())
                .unwrap();
            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            // Keep calls opaque: optimized direct calls can bypass the patched entry.
            let echo = std::hint::black_box(original_echo as unsafe extern "C" fn(Owned) -> Owned);
            let make = std::hint::black_box(original_make as unsafe extern "C" fn(usize) -> Owned);
            let result = echo(Owned {
                value: 17,
                padding: [1, 2, 3],
            });

            assert_eq!(SEEN.load(Ordering::SeqCst), 17);
            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
            drop(result);
            let result = make(23);
            assert_eq!(result.padding, [1, 2, 3]);
            assert_eq!(SEEN.load(Ordering::SeqCst), 23);
            assert_eq!(DROPS.load(Ordering::SeqCst), 1);
            drop(result);
            assert_eq!(DROPS.load(Ordering::SeqCst), 2);
        }
    }
}

mod custom_snapshots {
    use super::*;
    fixture_resolver!();
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    static SNAPSHOTS: AtomicUsize = AtomicUsize::new(0);
    static EXITS: AtomicUsize = AtomicUsize::new(0);
    static TWO_EXITS: AtomicUsize = AtomicUsize::new(0);
    static TWO_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static PANIC_EXIT: AtomicBool = AtomicBool::new(false);
    #[repr(C)]
    struct Owned {
        value: usize,
    }

    impl Drop for Owned {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    // Models a host-owned deep clone/retained native string snapshot.
    unsafe impl ObserverSnapshot for Owned {
        type Snapshot = Owned;
        fn snapshot(&self) -> Owned {
            SNAPSHOTS.fetch_add(1, Ordering::SeqCst);
            if self.value == 99 {
                panic!("snapshot preparation failed");
            }

            Owned { value: self.value }
        }

        fn borrow(snapshot: &Owned) -> &Owned {
            snapshot
        }
    }

    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Target)]
        unsafe fn call(value: Owned) -> Owned {}
        #[bind_resolved_func(Target)]
        unsafe fn two(first: Owned, second: Owned) {}
    }

    #[inline(never)]
    unsafe extern "C" fn original(mut value: Owned) -> Owned {
        value.value += 1;
        value
    }

    #[inline(never)]
    unsafe extern "C" fn original_two(first: Owned, second: Owned) {
        TWO_ORIGINAL.fetch_add(1, Ordering::SeqCst);
        drop((first, second));
    }

    #[bind_func_exit_listener(Native::call)]
    unsafe fn exit(value: &Owned, result: &Owned) {
        assert_eq!(value.value + 1, result.value);
        EXITS.fetch_add(1, Ordering::SeqCst);
        if PANIC_EXIT.load(Ordering::SeqCst) {
            panic!("snapshot observer failed");
        }
    }

    #[bind_func_exit_listener(Native::two)]
    unsafe fn exit_two(_first: &Owned, _second: &Owned, _result: &()) {
        TWO_EXITS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn custom_owned_snapshots_preserve_inputs_and_release_once_on_every_delivery_path() {
        unsafe {
            Native::__UNCHAINED_TARGET_call
                .binding()
                .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
                .unwrap();
            Native::__UNCHAINED_TARGET_two
                .binding()
                .initialize_address(NonZeroUsize::new(original_two as *const () as usize).unwrap())
                .unwrap();
            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            let call = std::hint::black_box(original as unsafe extern "C" fn(Owned) -> Owned);
            let result = call(Owned { value: 5 });
            assert_eq!(result.value, 6);
            assert_eq!(SNAPSHOTS.load(Ordering::SeqCst), 1);
            assert_eq!(DROPS.load(Ordering::SeqCst), 1); // Snapshot, not result.
            drop(result);
            PANIC_EXIT.store(true, Ordering::SeqCst);
            let result = call(Owned { value: 7 });
            assert_eq!(result.value, 8);
            assert_eq!(DROPS.load(Ordering::SeqCst), 3);
            drop(result);
            assert_eq!(DROPS.load(Ordering::SeqCst), 4);
        }

        let descriptor = Native::__UNCHAINED_TARGET_call;
        let value = Owned { value: 10 };
        let prepare = descriptor.exit_slot().get().unwrap();
        let observation = unsafe { prepare(&value) };
        drop(observation); // Skipped delivery still owns/releases exactly one snapshot.
        assert_eq!(DROPS.load(Ordering::SeqCst), 5);
        let observation = unsafe { prepare(&value) };
        descriptor.observe_entry(|_| unreachable!()); // Entry slot is absent.
                                                      // Install an entry slot only to exercise suppressed reentrant delivery.
        unsafe fn entry(_value: &Owned) {}
        unsafe {
            descriptor.entry_slot().register(entry, true, || true).unwrap();
        }

        descriptor.observe_entry(|_| unsafe {
            descriptor.finish_exit(observation, (&value as *const Owned).cast());
        });

        assert_eq!(DROPS.load(Ordering::SeqCst), 6);
        assert_eq!(EXITS.load(Ordering::SeqCst), 2);
        drop(value);
        assert_eq!(DROPS.load(Ordering::SeqCst), 7);
        let two = std::hint::black_box(original_two as unsafe extern "C" fn(Owned, Owned));
        unsafe {
            two(Owned { value: 1 }, Owned { value: 99 });
        }

        assert_eq!(TWO_ORIGINAL.load(Ordering::SeqCst), 1);
        assert_eq!(TWO_EXITS.load(Ordering::SeqCst), 0);
        assert_eq!(DROPS.load(Ordering::SeqCst), 10); // Partial snapshot + both real inputs.
    }
}

mod references {
    use super::*;
    fixture_resolver!();
    static SEEN: AtomicUsize = AtomicUsize::new(0);
    static RESULT: i32 = 42;
    struct Native;
    #[runtime_bindings]
    impl Native {
        #[bind_resolved_func(Target)]
        unsafe fn read<'a>(value: &'a mut i32) -> &'a mut i32 {}
        #[bind_resolved_func(Target)]
        unsafe fn reference() -> &'static i32 {}
        #[bind_resolved_func(Target)]
        unsafe fn pointer(value: *mut i32) -> *mut i32 {}
        #[bind_resolved_func(Target)]
        unsafe fn nothing(value: usize) {}
    }

    #[inline(never)]
    unsafe extern "C" fn original_read(value: &mut i32) -> &mut i32 {
        *value += 1;
        value
    }

    #[inline(never)]
    unsafe extern "C" fn original_reference() -> &'static i32 {
        &RESULT
    }

    #[inline(never)]
    unsafe extern "C" fn original_pointer(value: *mut i32) -> *mut i32 {
        value
    }

    #[inline(never)]
    unsafe extern "C" fn original_nothing(value: usize) {
        SEEN.fetch_add(value, Ordering::SeqCst);
    }

    #[bind_func_entry_listener(Native::read)]
    unsafe fn read_entry(value: &&mut i32) {
        SEEN.store(**value as usize, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(Native::reference)]
    unsafe fn reference_exit(result: &&i32) {
        assert_eq!(**result, 42);
    }

    #[bind_func_exit_listener(Native::pointer)]
    unsafe fn pointer_exit(value: &*mut i32, result: &*mut i32) {
        assert_eq!(value, result);
    }

    #[bind_func_exit_listener(Native::nothing)]
    unsafe fn void_exit(value: &usize, result: &()) {
        assert_eq!(*value, 3);
        assert_eq!(*result, ());
    }

    #[test]
    fn reference_inputs_and_void_pointer_reference_results_preserve_storage() {
        unsafe {
            for (binding, address) in [
                (
                    Native::__UNCHAINED_TARGET_read.binding(),
                    original_read as *const () as usize,
                ),
                (
                    Native::__UNCHAINED_TARGET_reference.binding(),
                    original_reference as *const () as usize,
                ),
                (
                    Native::__UNCHAINED_TARGET_pointer.binding(),
                    original_pointer as *const () as usize,
                ),
                (
                    Native::__UNCHAINED_TARGET_nothing.binding(),
                    original_nothing as *const () as usize,
                ),
            ] {
                binding.initialize_address(NonZeroUsize::new(address).unwrap()).unwrap();
            }

            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            let mut value = 7;
            assert_eq!(*original_read(&mut value), 8);
            assert_eq!(SEEN.load(Ordering::SeqCst), 7);
            assert!(std::ptr::eq(original_reference(), &RESULT));
            assert_eq!(original_pointer(&mut value), &mut value as *mut i32);
            original_nothing(3);
            assert_eq!(SEEN.load(Ordering::SeqCst), 10);
        }
    }
}
