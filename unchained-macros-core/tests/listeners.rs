use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

use unchained_macros_core::*;

struct ObservationTarget<E, X, R: AddressResolver> {
    pub resolver: &'static Resolver<R>,
    slot: CallbackSlot<Callback<std::convert::Infallible, E, X>>,
    attach: unsafe fn() -> Result<(), Box<dyn std::error::Error>>,
}

impl<E: Copy, X: Copy, R: AddressResolver> ObservationTarget<E, X, R> {
    /// # Safety
    /// Attachment and dispatch must honor E/X and preserve original execution.
    pub const unsafe fn new(
        resolver: &'static Resolver<R>,
        attach: unsafe fn() -> Result<(), Box<dyn std::error::Error>>,
    ) -> Self {
        Self {
            resolver,
            slot: CallbackSlot::new(),
            attach,
        }
    }
}

impl<E: Copy + Send + Sync + 'static, X: Copy + Send + Sync + 'static, R: AddressResolver> CallbackTarget
    for ObservationTarget<E, X, R>
{
    type Callback = std::convert::Infallible;
    type Entry = E;
    type Exit = X;
    fn slot(&self) -> &CallbackSlot<Callback<Self::Callback, E, X>> {
        &self.slot
    }
    fn resolver(&self) -> &'static dyn unchained_macros_core::__private::ResolverInfo {
        self.resolver
    }

    unsafe fn ensure_attached(
        &'static self,
        _: Callback<Self::Callback, E, X>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { (self.attach)() }
    }
}

impl<E: Copy + Send + Sync + 'static, X: Copy + Send + Sync + 'static, R: AddressResolver> SupportsEntry
    for ObservationTarget<E, X, R>
{
}

impl<E: Copy + Send + Sync + 'static, X: Copy + Send + Sync + 'static, R: AddressResolver> SupportsExit
    for ObservationTarget<E, X, R>
{
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

#[inline(never)]
unsafe extern "C" fn original(value: i32, output: *mut i32) -> i32 {
    EVENTS.lock().unwrap().push(('o', value));

    unsafe {
        *output += value;
    }

    value + 1
}

#[bind_func_entry_listener(Native::call)]
unsafe fn native_entry(value: &i32, _output: &*mut i32) {
    EVENTS.lock().unwrap().push(('e', *value));
}
#[bind_func_exit_listener(Native::call)]
unsafe fn native_exit(_result: &i32) {}
#[bind_func_detour(Native::call)]
unsafe fn replacement(value: i32, output: *mut i32) -> i32 {
    unsafe { Native::call(value, output) + 10 }
}
#[test]
fn entry_owner_rejects_other_kinds_and_preserves_bypass() {
    let binding = Native::__UNCHAINED_TARGET_call.binding();
    let entry = &__UNCHAINED_REGISTRATION_native_entry;
    let exit = &__UNCHAINED_REGISTRATION_native_exit;
    let hook = &__UNCHAINED_REGISTRATION_replacement;
    assert!(validate_callback_registrations_for::<Backend>(&SelectorSnapshot::default()).is_err());
    assert!(unsafe { entry.attach(true) }.is_err());
    unsafe {
        binding
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();
        entry.attach(false).unwrap();
    }
    assert!(unsafe { exit.attach(true) }.is_err());
    assert!(unsafe { hook.attach(true) }.is_err());
    entry.set_enabled(true);
    let mut output = 0;
    assert_eq!(unsafe { original(4, &mut output) }, 5);
    assert_eq!(unsafe { Native::call(6, &mut output) }, 7);
    assert_eq!(*EVENTS.lock().unwrap(), [('e', 4), ('o', 4), ('o', 6)]);
    assert_eq!(output, 10);
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

fn callback_panics_are_isolated() {
    assert!(isolate_observer(|| panic!("observer failure")).is_none());

    let slot = CallbackSlot::new();

    unsafe {
        slot.register(12_u32, false).unwrap();
    }

    assert!(slot.get().is_none());
}

#[test]

fn same_target_reentry_is_observed_and_panics_remain_isolated() {
    type Descriptor = ObservationTarget<unsafe fn(&u32), (), Backend>;

    unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    unsafe fn callback(_: &u32) {}

    static FIRST: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };

    static SECOND: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };

    unsafe {
        FIRST.register(Callback::Entry(callback), true, || true).unwrap();

        SECOND.register(Callback::Entry(callback), true, || true).unwrap();
    }

    FIRST.observe_entry(|_| {
        assert!(FIRST.observe_entry(|_| ()).is_some());

        assert!(SECOND.observe_entry(|_| ()).is_some());

        std::thread::spawn(|| assert!(FIRST.observe_entry(|_| ()).is_some()))
            .join()
            .unwrap();

        panic!("observer failure");
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

    type Descriptor = ObservationTarget<unsafe fn(&u32), unsafe fn(&bool), Backend>;

    unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> {
        ATTACHED.store(true, Ordering::SeqCst);

        Ok(())
    }

    static DESCRIPTOR: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };

    static EXIT_DESCRIPTOR: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
    struct VmOnly;

    impl VmOnly {
        #[allow(non_upper_case_globals)]
        const __UNCHAINED_TARGET_other: &'static Descriptor = &EXIT_DESCRIPTOR;
        #[allow(non_upper_case_globals)]
        const __UNCHAINED_TARGET_call: &'static Descriptor = &DESCRIPTOR;
    }

    #[bind_func_entry_listener(VmOnly::call)]

    unsafe fn entry(value: &u32) {
        CALLS.fetch_add(*value as usize, Ordering::SeqCst);
    }

    #[bind_func_exit_listener(VmOnly::other)]

    unsafe fn exit(result: &bool) {
        assert!(*result);

        CALLS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]

    fn observation_only_target_does_not_need_native_function_or_callable_method() {
        validate_callback_registrations_for::<Backend>(&SelectorSnapshot::default()).unwrap();

        for listener in listener_inventory_for::<Backend>() {
            unsafe {
                listener.attach(true).unwrap();
            }
        }

        assert!(ATTACHED.load(Ordering::SeqCst));

        assert_eq!(binding_inventory_for::<Backend>().count(), 0);

        assert_eq!(detour_inventory_for::<Backend>().count(), 0);

        DESCRIPTOR.observe_entry(|callback| unsafe { callback(&3) });

        EXIT_DESCRIPTOR.observe_exit(|callback| unsafe { callback(&true) });
        assert_eq!(CALLS.load(Ordering::SeqCst), 4);
    }
}

mod duplicate_discovery {

    use super::*;

    fixture_resolver!();

    static ATTACHED: AtomicBool = AtomicBool::new(false);

    static ATTACHMENTS: AtomicUsize = AtomicUsize::new(0);

    type Descriptor = ObservationTarget<unsafe fn(&u32), unsafe fn(&u32), Backend>;

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

    unsafe fn first_exit(_result: &u32) {}

    #[bind_func_exit_listener(VmOnly::call)]

    unsafe fn second_exit(_result: &u32) {}

    unsafe fn host_loop(snapshot: &SelectorSnapshot, preflight: bool) -> Result<(), Box<dyn std::error::Error>> {
        if preflight {
            validate_callback_registrations_for::<Backend>(snapshot)?;
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

        assert!(error.contains("Only one callback per method"), "{error}");

        assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 0);

        for listener in listener_inventory_for::<Backend>() {
            assert!(!listener.initialized());

            assert!(!listener.enabled());
        }

        // Even a host without preflight must not skip a loser as "initialized".
        for listener in listener_inventory_for::<Backend>().filter(|listener| listener.name == "first_entry") {
            unsafe {
                listener.attach(true).unwrap();
            }
        }

        assert_eq!(ATTACHMENTS.load(Ordering::SeqCst), 1);

        let error = unsafe { host_loop(&snapshot, false) }.unwrap_err().to_string();

        assert!(error.contains("Only one"), "{error}");

        for listener in listener_inventory_for::<Backend>() {
            if listener.name != "first_entry" {
                assert!(!listener.initialized());

                assert!(!listener.enabled());

                listener.set_enabled(false);
            }
        }

        for listener in listener_inventory_for::<Backend>().filter(|listener| listener.name == "first_entry") {
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
                assert!(listener.attach(true).is_err());
            }

            assert!(!off.enabled());

            assert!(!gated.enabled());

            assert_eq!(original_off(), 11);

            assert_eq!(original_gated(), 12);

            assert_eq!(ENTRIES.load(Ordering::SeqCst), 0);

            off.set_enabled(true);

            assert!(off.enabled());

            assert_eq!(original_off(), 99);

            assert_eq!(ENTRIES.load(Ordering::SeqCst), 0);

            assert_eq!(Native::off(), 11);

            assert_eq!(Native::gated(), 12);

            assert_eq!(ENTRIES.load(Ordering::SeqCst), 0);

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

    unsafe fn exit(_result: &usize) {
        EXITS.fetch_add(1, Ordering::SeqCst);
    }

    #[bind_func_detour(Native::call, condition)]

    unsafe fn detour(value: usize) -> usize {
        unsafe { Native::call(value) + 10 }
    }

    #[test]

    fn conflicting_callbacks_do_not_evaluate_conditions() {
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

            assert!(exit.attach(true).is_err());
            assert!(detour_inventory_for::<Backend>().next().unwrap().attach(true).is_err());
            assert_eq!(CONDITIONS.load(Ordering::SeqCst), 0);
            assert_eq!(original(7), 8);
            assert_eq!(ORIGINAL.load(Ordering::SeqCst), 1);
            assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
            assert_eq!(EXITS.load(Ordering::SeqCst), 0);
        }
    }
}

mod local_hygiene {

    use super::*;

    fixture_resolver!();

    static EXIT: AtomicUsize = AtomicUsize::new(0);

    static OBSERVATIONS: AtomicUsize = AtomicUsize::new(0);

    struct Native;

    #[runtime_bindings]

    impl Native {
        #[bind_resolved_func(Target)]

        unsafe fn call(
            __unchained_callback: usize,
            __unchained_result: usize,
            __unchained_call: usize,
            __unchained_native_call: usize,
        ) -> usize {
        }
    }

    #[inline(never)]
    unsafe extern "C" fn original(a: usize, b: usize, c: usize, d: usize) -> usize {
        a + 2 * b + 3 * c + 4 * d
    }

    #[bind_func_exit_listener(Native::call)]

    unsafe fn observe_result(return_value: &usize) {
        EXIT.store(*return_value, Ordering::SeqCst);
        OBSERVATIONS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]

    fn user_parameter_names_do_not_alias_generated_locals() {
        unsafe {
            Native::__UNCHAINED_TARGET_call
                .binding()
                .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
                .unwrap();

            validate_callback_registrations_for::<Backend>(&SelectorSnapshot::default()).unwrap();

            for listener in listener_inventory_for::<Backend>() {
                listener.attach(true).unwrap();
            }

            let expected = 30;
            assert_eq!(original(1, 2, 3, 4), expected);
            assert_eq!(EXIT.load(Ordering::SeqCst), expected);

            assert_eq!(OBSERVATIONS.load(Ordering::SeqCst), 1);

            assert_eq!(Native::call(1, 2, 3, 4), expected);

            assert_eq!(OBSERVATIONS.load(Ordering::SeqCst), 1);
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

    unsafe fn exit(result: &Owned) {
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

    unsafe fn pointer_exit(result: &*mut i32) {
        assert!(!result.is_null());
    }

    #[bind_func_exit_listener(Native::nothing)]

    unsafe fn void_exit(result: &()) {
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
