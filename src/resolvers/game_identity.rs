//! Opt-in native browser admission. Native memory is only used on the game thread.
use super::unchained_integration::run_on_game_thread;
use crate::{
    features::{game_join_protocol as protocol, tokio_runtime::TOKIO_RUNTIME},
    tools::hook_globals::cli_args,
    ue::{APlayerController, FGuid, FName, FString, FWeakObjectPtr, UObjectBase},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    sync::atomic::{AtomicU64, Ordering},
};

static TRAVEL_EPOCH: AtomicU64 = AtomicU64::new(0);
static CLIENT_SLOT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

pub fn configured_backend() -> Option<String> {
    let args = cli_args();
    if !args.validated_player_identity {
        return None;
    }
    protocol::trusted_backend(args.server_browser_backend.as_deref()?)
}

pub fn admission_available() -> bool {
    o_ValidatedConfirmJoin.is_enabled()
        && o_IdentityRequireFreshJoin.is_enabled()
        && o_IdentityClearSharedId.is_enabled()
        && o_IdentityRejectPlayer.is_enabled()
        && super::identity_authority::available()
}

define_pattern_resolver!(IdentityRequireFreshJoin, [
    "48 89 54 24 10 48 89 4C 24 08 55 53 56 57 41 54 41 56 41 57 48 8D AC 24 60 FE FF FF 48 81 EC A0 02 00 00"
]);
define_pattern_resolver!(IdentityClearSharedId, [
    "48 89 5C 24 10 57 48 83 EC 30 48 8B D9 C7 41 20 00 00 00 00 48 83 C1 18 48 8B FA 83 79 0C 00 74 07 33 D2"
]);
CREATE_HOOK!(IdentityClearSharedId, ACTIVE, (), (wrapper: *mut crate::game::chivalry2::FUniqueNetIdRepl, empty: *const [*const c_void; 2]), {});
define_pattern_resolver!(IdentityRejectPlayer, [
    "48 8B C4 4C 89 48 20 55 48 8D 68 E8 48 81 EC 10 01 00 00 48 89 58 08 48 63 5A 08 48 89 78 E8 48 8B 3A 4C 89 60 E0 45 33 E4 4C"
]);
CREATE_HOOK!(IdentityRejectPlayer, ACTIVE, (), (state: *const u8, playfab: *const FString, unique: *const FString, ticket: *const FString), {});

#[derive(Clone, Copy)]
struct ConnectionWitness {
    connection: FWeakObjectPtr,
    address: *const UObjectBase,
    state: FWeakObjectPtr,
}

#[derive(Clone, Copy)]
struct AdmissionContext {
    witness: ConnectionWitness,
    started: bool,
}

unsafe fn connection_for(state: *const u8) -> *const UObjectBase {
    let get: unsafe extern "C" fn(*const u8) -> *const UObjectBase =
        unsafe { std::mem::transmute(slot(state.cast(), 0x4b0 / 8)) };
    unsafe { get(state) }
}

impl ConnectionWitness {
    unsafe fn current(self) -> bool {
        let array = unsafe { crate::globals().guobject_array_unchecked() };
        let Some(connection) = (unsafe { self.connection.get(array) }) else {
            return false;
        };
        let Some(state) = (unsafe { self.state.get(array) }) else {
            return false;
        };
        array.is_live(connection)
            && array.is_live(state)
            && std::ptr::eq(connection, self.address)
            && unsafe { connection_for((state as *const UObjectBase).cast()) } == self.address
    }
}

CREATE_HOOK!(IdentityRequireFreshJoin, ACTIVE, NONE, (), (mode: *mut c_void, controller: *const u8), {
    if !cli_args().validated_player_identity || controller.is_null() {
        return CALL_ORIGINAL!(IdentityRequireFreshJoin(mode, controller));
    }
    let state = unsafe { controller.add(0x260).cast::<*const u8>().read() };
    if state.is_null() { return CALL_ORIGINAL!(IdentityRequireFreshJoin(mode, controller)); }
    let connection = unsafe { connection_for(state) };
    if connection.is_null() { return CALL_ORIGINAL!(IdentityRequireFreshJoin(mode, controller)); }
    let array = unsafe { crate::globals().guobject_array_unchecked() };
    let already_verified = VERIFIED_CONNECTIONS.with(|known| {
        let mut known = known.borrow_mut();
        known.retain(|_, weak| unsafe { weak.get(array) }.is_some_and(|object| array.is_live(object)));
        known.get(&connection).is_some_and(|weak| unsafe { weak.get(array) }.is_some_and(|object| std::ptr::eq(object, connection)))
    });
    if already_verified { return CALL_ORIGINAL!(IdentityRequireFreshJoin(mode, controller)); }
    // Restored inactive PlayerStates otherwise skip confirmation. Invalidate
    // only their cached proof through FUniqueNetIdRepl::SetUniqueNetId, then
    // let native initialization build its normal weak-object delegate again.
    let empty = [std::ptr::null(); 2];
    CALL_ORIGINAL!(IdentityClearSharedId(unsafe { state.add(0x5c8) } as *mut _, &empty));
    let witness = ConnectionWitness { connection: FWeakObjectPtr::new(unsafe { &*connection }), address: connection,
        state: FWeakObjectPtr::new(unsafe { &*state.cast::<UObjectBase>() }) };
    let previous = INITIALIZING.with(|current| current.replace(Some(AdmissionContext { witness, started: false })));
    CALL_ORIGINAL!(IdentityRequireFreshJoin(mode, controller));
    let started = INITIALIZING.with(|current| current.replace(previous)).is_some_and(|context| context.started);
    if !started && unsafe { witness.current() } {
        // No supported confirmation path ran. Use the native rejection helper;
        // never leave an opted-in unverified connection admitted silently.
        let empty = FString::new();
        CALL_ORIGINAL!(IdentityRejectPlayer(state, &empty, &empty, &empty));
    }
});

// Exact current-build confirmation prologue. Never guess a delegate ABI on an
// unrecognized build. The normal native path remains unchanged without opt-in.
define_pattern_resolver!(ValidatedConfirmJoin, [
    "48 89 5C 24 08 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 00 FF FF FF 48 81 EC 00 02 00 00 48 8B 05 E2 83 4F 04 48 33 C4 48 89 85 F0 00 00 00 48 8B 85 60 01 00 00 33 F6 4D 8B F0"
]);

define_pattern_resolver!(IdentitySubsystem, [
    "48 89 5C 24 08 57 48 83 EC 20 65 48 8B 04 25 58 00 00 00 48 8B F9 8B 15 ?? ?? ?? ?? B9 78 00 00 00 48 8B 14 D0 8B 04 11 39 05 ?? ?? ?? ?? 7F 31"
]);
CREATE_HOOK!(IdentitySubsystem, ACTIVE, *const u8, (name: *const FName), {});

#[repr(C)]
#[derive(Default)]
pub struct NativeDelegate {
    object: *mut c_void,
    allocation_units: i32,
    padding: i32,
}

impl NativeDelegate {
    unsafe fn clone_from(source: *const Self) -> Option<Self> {
        let source = unsafe { source.as_ref()? };
        if source.allocation_units == 0 || source.object.is_null() {
            return None;
        }
        let mut result = Self::default();
        let clone: unsafe extern "C" fn(*mut c_void, *mut Self) =
            unsafe { std::mem::transmute(slot(source.object, 8)) };
        unsafe { clone(source.object, &mut result) };
        (!result.object.is_null()).then_some(result)
    }

    unsafe fn complete(&mut self, accepted: bool) -> bool {
        if self.object.is_null() {
            return false;
        }
        let bound: unsafe extern "C" fn(*mut c_void) -> bool =
            unsafe { std::mem::transmute(slot(self.object, 5)) };
        if unsafe { bound(self.object) } {
            let execute: unsafe extern "C" fn(*mut c_void, bool) =
                unsafe { std::mem::transmute(slot(self.object, 10)) };
            // The native lambda first resolves its captured weak PlayerState.
            unsafe { execute(self.object, accepted) };
            return true;
        }
        false
    }
}

impl Drop for NativeDelegate {
    fn drop(&mut self) {
        if self.object.is_null() {
            return;
        }
        unsafe {
            let destroy: unsafe extern "C" fn(*mut c_void, u8) =
                std::mem::transmute(slot(self.object, 7));
            destroy(self.object, 0);
            crate::globals().gmalloc().free(self.object);
        }
    }
}

unsafe fn slot(object: *const c_void, index: usize) -> usize {
    unsafe { object.cast::<*const usize>().read().add(index).read() }
}

thread_local! {
    static INITIALIZING: Cell<Option<AdmissionContext>> = const { Cell::new(None) };
    static VERIFIED_CONNECTIONS: RefCell<HashMap<*const UObjectBase, FWeakObjectPtr>> = RefCell::new(HashMap::new());
}

struct PendingAdmission {
    delegate: NativeDelegate,
    witness: Option<ConnectionWitness>,
}

impl PendingAdmission {
    unsafe fn complete(&mut self, mut accepted: bool) -> bool {
        if self
            .witness
            .is_some_and(|witness| unsafe { !witness.current() })
        {
            return false;
        }
        let array = unsafe { crate::globals().guobject_array_unchecked() };
        VERIFIED_CONNECTIONS.with(|known| {
            let mut known = known.borrow_mut();
            known.retain(|_, weak| {
                unsafe { weak.get(array) }.is_some_and(|object| array.is_live(object))
            });
            if known.len() >= 1024 {
                accepted = false;
            }
        });
        let applied = unsafe { self.delegate.complete(accepted) };
        if accepted && applied {
            if let Some(witness) = self.witness {
                VERIFIED_CONNECTIONS.with(|known| {
                    known
                        .borrow_mut()
                        .insert(witness.address, witness.connection);
                });
            }
        }
        accepted && applied
    }
}

CREATE_HOOK!(ValidatedConfirmJoin, ACTIVE, NONE, bool, (
    context: *mut c_void, playfab: *const FString, unique: *const FString,
    ticket: *const FString, callback: *const NativeDelegate
), {
    let configured = cli_args().validated_player_identity;
    let raw = unsafe { ticket.as_ref() }.and_then(|value| value.copy_to_string().ok()).unwrap_or_default();
    if !configured && !raw.starts_with(protocol::PREFIX) {
        return CALL_ORIGINAL!(ValidatedConfirmJoin(context, playfab, unique, ticket, callback));
    }
    let Some(delegate) = (unsafe { NativeDelegate::clone_from(callback) }) else { return false; };
    let witness = INITIALIZING.with(|current| {
        let mut context = current.get()?;
        context.started = true;
        current.set(Some(context));
        Some(context.witness)
    });
    let mut delegate = PendingAdmission { delegate, witness };
    // This path never redeems opaque grants, including after JWT rejection.
    let claims = configured_backend().and_then(|_| {
        if !admission_available() || !protocol::admission_ticket(&raw) { return None; }
        let registration = crate::globals().registration.lock().ok()?;
        registration.as_ref()?.consume_admission(&raw).ok()
    }).filter(|claims| {
        unsafe { playfab.as_ref() }.and_then(|value| value.copy_to_string().ok())
            .is_some_and(|claimed| claimed == claims.sub)
    });
    let ids = claims.as_ref().and_then(|claims| unsafe { super::identity_authority::VerifiedIds::prepare(claims) });
    let ready = match (delegate.witness, ids.as_ref()) {
        (Some(witness), Some(ids)) if unsafe { witness.current() } => {
            let array = unsafe { crate::globals().guobject_array_unchecked() };
            if let Some(state) = unsafe { witness.state.get(array) } {
                unsafe { ids.assign(state as *const UObjectBase as *mut u8) };
                unsafe { witness.current() }
            } else { false }
        }
        _ => false,
    };
    let applied = unsafe { delegate.complete(ready) };
    sinfo!("Offline game join validation completed: accepted={}", applied);
    true
});

/// Returns true when the travel was suspended or rejected. Never blocks on HTTP.
pub unsafe fn intercept_travel(
    controller: *mut APlayerController,
    url: *const FString,
    travel_type: u8,
    seamless: bool,
    guid: *const FGuid,
) -> bool {
    let epoch = TRAVEL_EPOCH.fetch_add(1, Ordering::Relaxed) + 1;
    if !cli_args().validated_player_identity {
        return false;
    }
    let Some(raw) = (unsafe { url.as_ref() }).and_then(|value| value.copy_to_string().ok()) else {
        return true;
    };
    let Some(target) = protocol::target(&raw) else {
        return false;
    };
    let Some(claimed) = protocol::claimed_account(&raw).map(str::to_owned) else {
        return true;
    };
    if !o_IdentitySubsystem.is_enabled() || !o_ValidatedConfirmJoin.is_enabled() {
        swarn!("Game identity adapter unavailable for this executable; browser travel cancelled");
        return true;
    }
    let Some(backend) = configured_backend() else {
        swarn!("Game identity trust configuration rejected; browser travel cancelled");
        return true;
    };
    let Ok(permit) = CLIENT_SLOT.try_acquire() else {
        swarn!("Game identity request already pending; browser travel cancelled");
        return true;
    };
    let Some(object) = (unsafe { controller.cast::<UObjectBase>().as_ref() }) else {
        return true;
    };
    let weak = FWeakObjectPtr::new(object);
    let guid = crate::ue::try_read(guid.cast::<[u8; 16]>());
    let Some(guid) = guid else {
        return true;
    };
    let Some(credential) = (unsafe { native_credential() }) else {
        swarn!("Native game credential unavailable; browser travel cancelled");
        return true;
    };
    let Some(eos_id_token) = (unsafe { super::eos_identity::copy_local_id_token() }) else {
        swarn!("EOS Connect identity token unavailable; browser travel cancelled");
        return true;
    };
    let world = crate::globals()
        .world()
        .map(|world| FWeakObjectPtr::new(unsafe { &*world.cast::<UObjectBase>() }));
    drop(TOKIO_RUNTIME.spawn(async move {
        let ticket =
            crate::features::playfab_proof::issue_admission(&backend, target, &claimed, credential, eos_id_token).await;
        run_on_game_thread(move || {
            let _permit = permit;
            if TRAVEL_EPOCH.load(Ordering::Relaxed) != epoch {
                sinfo!("Validated browser travel superseded by a newer travel request");
                return;
            }
            let Some(ticket) = ticket else {
                swarn!("Game identity verification failed; browser travel cancelled");
                return;
            };
            unsafe {
                let array = crate::globals().guobject_array_unchecked();
                let Some(object) = weak.get(array) else {
                    swarn!("Validated browser travel cancelled: controller no longer exists");
                    return;
                };
                if !array.is_live(object) {
                    swarn!("Validated browser travel cancelled: controller is no longer live");
                    return;
                }
                if object.object_flags.intersects(
                    crate::ue::EObjectFlags::RF_BeginDestroyed
                        | crate::ue::EObjectFlags::RF_FinishDestroyed,
                ) {
                    swarn!("Validated browser travel cancelled: controller is being destroyed");
                    return;
                }
                if world.is_some_and(|world| {
                    world.get(array).is_none_or(|object| {
                        !array.is_live(object)
                            || crate::globals()
                                .world()
                                .map(|world| world.cast::<UObjectBase>() as *const UObjectBase)
                                != Some(object as *const UObjectBase)
                    })
                }) {
                    swarn!("Validated browser travel cancelled: world changed during verification");
                    return;
                }
                let mut url = FString::from(protocol::replace_ticket(&raw, &ticket).as_str());
                let mut guid = guid;
                // Resume below the interceptor; no second issuance or epoch change.
                sinfo!("Resuming validated browser travel");
                crate::globals().set_world(std::ptr::null_mut());
                super::etc_hooks::o_ClientTravelInternal.call(
                    object as *const UObjectBase as *mut APlayerController,
                    &mut url,
                    travel_type,
                    seamless,
                    guid.as_mut_ptr().cast(),
                );
            }
        });
    }));
    true
}

unsafe fn native_credential() -> Option<String> {
    let shared = unsafe { identity_interface("PlayFab") }?;
    let object = shared.object;
    let auth: unsafe extern "C" fn(*const c_void, *mut FString, i32) =
        unsafe { std::mem::transmute(slot(object, 38)) };
    let mut token = FString::new();
    unsafe { auth(object, &mut token, 0) };
    token.copy_to_string().ok().filter(|value| !value.is_empty())
}

unsafe fn identity_interface(subsystem_name: &str) -> Option<NativeIdentityRef> {
    if !o_IdentitySubsystem.is_enabled() { return None; }
    let name = unsafe { FName::new(&FString::from(subsystem_name)) };
    let subsystem = CALL_ORIGINAL!(IdentitySubsystem(&name));
    if subsystem.is_null() {
        return None;
    }
    let identity: unsafe extern "C" fn(*const u8, *mut NativeIdentityRef) =
        unsafe { std::mem::transmute(slot(subsystem.cast(), 16)) };
    let mut shared = NativeIdentityRef {
        object: std::ptr::null(),
        reference: std::ptr::null_mut(),
    };
    unsafe { identity(subsystem, &mut shared) };
    if shared.object.is_null() {
        return None;
    }
    Some(shared)
}

/// Build native typed IDs through IOnlineIdentity::CreateUniquePlayerId(FString),
/// then check the native validity and round-trip value before assigning fields.
pub(crate) unsafe fn create_native_id(subsystem: &str, value: &str) -> Option<NativeIdentityRef> {
    let identity = unsafe { identity_interface(subsystem) }?;
    // Native PlayerState initialization calls the FString overload at +0x100.
    // MSVC's overload slots do not follow declaration order in the UE header.
    let create: unsafe extern "C" fn(*const c_void, *mut NativeIdentityRef, *const FString) =
        unsafe { std::mem::transmute(slot(identity.object, 32)) };
    let mut result = NativeIdentityRef { object: std::ptr::null(), reference: std::ptr::null_mut() };
    unsafe { create(identity.object, &mut result, &FString::from(value)) };
    if result.object.is_null() { return None; }
    let valid: unsafe extern "C" fn(*const c_void) -> bool =
        unsafe { std::mem::transmute(slot(result.object, 5)) };
    if !unsafe { valid(result.object) } { return None; }
    let to_string: unsafe extern "C" fn(*const c_void, *mut FString) =
        unsafe { std::mem::transmute(slot(result.object, 6)) };
    let mut text = FString::new();
    unsafe { to_string(result.object, &mut text) };
    (text.copy_to_string().ok().as_deref() == Some(value)).then_some(result)
}

/// Owned result of IOnlineSubsystem::GetIdentityInterface; never leaves the game thread.
#[repr(C)]
pub(crate) struct NativeIdentityRef {
    object: *const c_void,
    reference: *mut u8,
}

impl Drop for NativeIdentityRef {
    fn drop(&mut self) {
        if self.reference.is_null() {
            return;
        }
        unsafe {
            let strong = &*self.reference.add(8).cast::<std::sync::atomic::AtomicI32>();
            if strong.fetch_sub(1, Ordering::AcqRel) == 1 {
                let destroy: unsafe extern "C" fn(*mut u8) =
                    std::mem::transmute(slot(self.reference.cast(), 0));
                destroy(self.reference);
                let weak = &*self
                    .reference
                    .add(12)
                    .cast::<std::sync::atomic::AtomicI32>();
                if weak.fetch_sub(1, Ordering::AcqRel) == 1 {
                    let free: unsafe extern "C" fn(*mut u8, u8) =
                        std::mem::transmute(slot(self.reference.cast(), 1));
                    free(self.reference, 1);
                }
            }
        }
    }
}
