use std::{hint::black_box, num::NonZeroUsize};
use unchained_macros_core::{AddressResolver, FunctionBinding, NativeHook, Resolver};

struct ExplicitAddress;

impl AddressResolver for ExplicitAddress {
    type Context = NonZeroUsize;

    unsafe fn resolve(&self, address: &Self::Context) -> Result<NonZeroUsize, String> {
        Ok(*address)
    }
}

static RESOLVER: Resolver<ExplicitAddress> = Resolver {
    name: "explicit",
    implementation: ExplicitAddress,
    filter: None,
};

static OWNER: FunctionBinding<ExplicitAddress> = FunctionBinding::new("owner", &RESOLVER);

static ALIAS: FunctionBinding<ExplicitAddress> = FunctionBinding::new("late alias", &RESOLVER);

static HOOK: NativeHook = NativeHook::new();

type Call = unsafe extern "C" fn(usize) -> usize;

#[inline(never)]

unsafe extern "C" fn original(n: usize) -> usize {
    black_box(n).wrapping_add(1)
}

unsafe extern "C" fn replacement(n: usize) -> usize {
    n.wrapping_add(10)
}

#[test]

fn unlisted_owner_and_late_alias_receive_original_trampoline() {
    let entry = black_box(original as Call);

    let address = NonZeroUsize::new(entry as usize).unwrap();

    unsafe {
        OWNER.initialize_address(address).unwrap();

        HOOK.attach(&OWNER, replacement as *const ()).unwrap();

        let bypass = OWNER.address().unwrap();

        assert_ne!(bypass, address);

        ALIAS.initialize_address(address).unwrap();

        assert_eq!(ALIAS.address(), Some(bypass));

        let call_original: Call = std::mem::transmute(bypass.get());

        assert_eq!(entry(1), 11);

        assert_eq!(call_original(1), 2);
    }
}
