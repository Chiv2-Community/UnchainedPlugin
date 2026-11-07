//! Ticket lifecycle for Redpoint's throwaway token check. No engine dependencies.
use std::cell::Cell;

pub(crate) fn signature_required(name: &str, steam: bool, executable_crc: u32) -> bool {
    match name {
        "RedpointSteamLoginProbe" | "SteamIdentityGetAuthToken" => {
            steam && executable_crc == 0x04d195bc
        }
        _ => true,
    }
}

thread_local! {
    static PROBE: Cell<Option<(usize, i32)>> = const { Cell::new(None) };
}

pub(super) struct ProbeScope(Option<(usize, i32)>);

impl ProbeScope {
    pub(super) fn enter(identity: usize, local_user: i32) -> Self {
        Self(PROBE.with(|probe| probe.replace(Some((identity, local_user)))))
    }
}

impl Drop for ProbeScope {
    fn drop(&mut self) {
        PROBE.with(|probe| probe.set(self.0));
    }
}

// Consume before calling Steam: later authentication callbacks must not inherit it.
pub(super) fn take_probe(identity: usize, local_user: i32) -> bool {
    PROBE.with(|probe| {
        if probe.get() == Some((identity, local_user)) {
            probe.set(None);
            true
        } else {
            false
        }
    })
}

pub(super) fn temporary_token(
    get_ticket: impl FnOnce(&mut [u8], &mut u32) -> u32,
    cancel_ticket: impl FnOnce(u32),
) -> Vec<u16> {
    let mut buffer = [0u8; 1024];
    let mut length = 0;
    let handle = get_ticket(&mut buffer, &mut length);
    if handle == 0 {
        return Vec::new();
    }

    // This caller only checks the string's length. No ticket leaves this check.
    // Cancel even if Steam returned an empty or unusable payload.
    cancel_ticket(handle);
    if length == 0 || length as usize > buffer.len() {
        return Vec::new();
    }
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    buffer[..length as usize]
        .iter()
        .flat_map(|byte| {
            [
                HEX[(byte >> 4) as usize] as u16,
                HEX[(byte & 15) as usize] as u16,
            ]
        })
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
pub(super) fn signature_matches(code: &[u8], signature: &str) -> bool {
    let mut count = 0;
    for (index, token) in signature.split_ascii_whitespace().enumerate() {
        let Some(byte) = code.get(index) else {
            return false;
        };
        if token != "??" && u8::from_str_radix(token, 16).ok() != Some(*byte) {
            return false;
        }
        count += 1;
    }
    count != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_signatures_are_required_only_for_supported_steam_build() {
        for name in ["RedpointSteamLoginProbe", "SteamIdentityGetAuthToken"] {
            assert!(signature_required(name, true, 0x04d195bc));
            assert!(!signature_required(name, false, 0x04d195bc));
            assert!(!signature_required(name, true, 0));
        }
        assert!(signature_required("ExistingResolver", false, 0));
    }

    #[test]
    fn scope_is_identity_and_user_specific_and_one_shot() {
        let _scope = ProbeScope::enter(1, 0);
        assert!(!take_probe(2, 0));
        assert!(!take_probe(1, 1));
        assert!(take_probe(1, 0));
        assert!(!take_probe(1, 0));
    }

    #[test]
    fn scope_restores_nested_and_unconsumed_state() {
        let outer = ProbeScope::enter(1, 0);
        {
            let _inner = ProbeScope::enter(2, 0);
            assert!(take_probe(2, 0));
        }
        assert!(take_probe(1, 0));
        drop(outer);
        assert!(!take_probe(1, 0));
        {
            let _unused = ProbeScope::enter(1, 0);
        }
        assert!(!take_probe(1, 0));
    }

    #[test]
    fn scope_does_not_cross_threads() {
        let _scope = ProbeScope::enter(1, 0);
        assert!(!std::thread::spawn(|| take_probe(1, 0)).join().unwrap());
        assert!(take_probe(1, 0));
    }

    #[test]
    fn ordinary_calls_and_unwound_scopes_are_not_intercepted() {
        assert!(!take_probe(1, 0));
        let _ = std::panic::catch_unwind(|| {
            let _scope = ProbeScope::enter(1, 0);
            panic!("scope cleanup");
        });
        assert!(!take_probe(1, 0));
    }

    #[test]
    fn successful_probe_cancels_once_and_preserves_token() {
        let cancelled = Cell::new(0);
        let token = temporary_token(
            |buffer, length| {
                buffer[..2].copy_from_slice(&[0xab, 0x01]);
                *length = 2;
                42
            },
            |handle| {
                assert_eq!(handle, 42);
                cancelled.set(cancelled.get() + 1);
            },
        );
        assert_eq!(cancelled.get(), 1);
        assert_eq!(token, vec![65, 66, 48, 49, 0]);
    }

    #[test]
    fn invalid_handle_is_not_cancelled_or_reported_as_success() {
        assert!(temporary_token(
            |_, length| {
                *length = 10;
                0
            },
            |_| panic!()
        )
        .is_empty());
    }

    #[test]
    fn valid_handle_with_bad_length_is_still_cancelled() {
        for length in [0, 1025, u32::MAX] {
            let cancelled = Cell::new(false);
            assert!(temporary_token(
                |_, out| {
                    *out = length;
                    42
                },
                |_| cancelled.set(true)
            )
            .is_empty());
            assert!(cancelled.get());
        }
    }

    #[test]
    fn repeated_probes_do_not_accumulate_tickets() {
        let outstanding = Cell::new(0);
        for _ in 0..100 {
            temporary_token(
                |_, length| {
                    outstanding.set(outstanding.get() + 1);
                    *length = 1024;
                    42
                },
                |_| outstanding.set(outstanding.get() - 1),
            );
            assert_eq!(outstanding.get(), 0);
        }
    }

    #[test]
    fn signatures_reject_stale_offsets_and_changed_instructions() {
        assert!(signature_matches(&[0x48, 0x8b, 0xff], "48 8B ??"));
        assert!(!signature_matches(&[0x49, 0x8b, 0xff], "48 8B ??"));
        assert!(!signature_matches(&[0x48], "48 8B ??"));
        assert!(!signature_matches(&[0x48], ""));
        assert!(!signature_matches(&[0x48], "XX"));
    }
}
