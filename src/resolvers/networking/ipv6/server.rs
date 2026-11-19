//! Server IPv6 ping socket and sender preservation.
use std::cell::Cell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::game::sockets::{self, InternetAddr, NativeSocket, SocketSubsystem, UdpSocketBuilder};
use crate::tools::hook_globals::cli_args;
use crate::ue::FName;

static PING_SOCKET: AtomicPtr<NativeSocket> = AtomicPtr::new(std::ptr::null_mut());
thread_local! {
    static RECEIVED_PEER: Cell<*mut InternetAddr> = const { Cell::new(std::ptr::null_mut()) };
    static REPLY_PEER: Cell<*mut InternetAddr> = const { Cell::new(std::ptr::null_mut()) };
}

fn enabled() -> bool {
    cli_args().is_server() && cli_args().use_ipv6
}

define_pattern_resolver!(NativeSocketSubsystemGet, [
    "40 57 48 83 EC 20 65 48 8B 04 25 58 00 00 00 48 8B F9 8B 15 ?? ?? ?? ?? B9 ?? ?? ?? ?? 48 8B 14 D0 8B 04 11 39 05 ?? ?? ?? ?? 7F 16 48 8B 0D ?? ?? ?? ?? 48 8B 17 48 8B 01 48 83 C4 20 5F 48 FF 60 48"
]);
CREATE_HOOK!(NativeSocketSubsystemGet, CALLED, *mut SocketSubsystem, (name: *const FName), {});

define_pattern_resolver!(NativeUdpSocketBuilderBuild, [
    "40 55 53 41 54 48 8D 6C 24 B9 48 81 EC D0 00 00 00 4C 8B E1 48 8D 15 ?? ?? ?? ?? 48 8D 4D 0F"
]);
CREATE_HOOK!(NativeUdpSocketBuilderBuild, { enabled }, NONE, *mut NativeSocket, (
    builder: *const UdpSocketBuilder
), {
    if builder.is_null() {
        return CALL_ORIGINAL!(NativeUdpSocketBuilderBuild(builder));
    }
    let description = unsafe { (*builder).description.to_string() };
    if description == "ue4-udp-ping" {
        let windows = unsafe { sockets::name(&[87, 73, 78, 68, 79, 87, 83, 0]) };
        let subsystem = CALL_ORIGINAL!(NativeSocketSubsystemGet(&windows));
        if let Some(socket) = unsafe { sockets::build_ipv6_ping(subsystem, &*builder) } {
            PING_SOCKET.store(socket, Ordering::Release);
            sinfo!("Native UE ping socket bound to [::]:{} (dual-stack)", unsafe { (*builder).port });
            return socket;
        }
        serror!("Native IPv6 ping socket creation failed; retaining the original IPv4 builder");
    }
    CALL_ORIGINAL!(NativeUdpSocketBuilderBuild(builder))
});

define_pattern_resolver!(
    NativeSocketRecvFrom,
    ["48 89 5C 24 08 48 89 74 24 10 57 48 83 EC 30 44 8B 54 24 68 49 8B D9 8B 79 08"]
);
CREATE_HOOK!(NativeSocketRecvFrom, { enabled }, NONE, bool, (
    socket: *mut NativeSocket, data: *mut u8, size: i32, read: *mut i32,
    source: *mut InternetAddr, flags: i32
), {
    let tracked = socket == PING_SOCKET.load(Ordering::Acquire);
    if tracked {
        RECEIVED_PEER.with(|peer| peer.set(std::ptr::null_mut()));
    }
    let result = CALL_ORIGINAL!(NativeSocketRecvFrom(socket, data, size, read, source, flags));
    if tracked && result && !read.is_null() && unsafe { *read } > 0 {
        RECEIVED_PEER.with(|peer| peer.set(source));
    }
    result
});

// FUdpSocketReceiver invokes this callback synchronously, before releasing its
// TSharedRef<FInternetAddr>. Preserve that address instead of the lossy
// FIPv4Endpoint that the receiver constructs for its delegate.
define_pattern_resolver!(
    NativePingResponse,
    ["48 8B C4 53 57 41 55 41 57 48 83 EC 58 48 8B 1A 4D 8B F8 48 8B FA 4C 8B E9 48 85 DB"]
);
struct ReplyScope(*mut InternetAddr);
impl ReplyScope {
    fn enter() -> Self {
        let received = RECEIVED_PEER.with(|peer| peer.replace(std::ptr::null_mut()));
        Self(REPLY_PEER.with(|peer| peer.replace(received)))
    }
}
impl Drop for ReplyScope {
    fn drop(&mut self) {
        REPLY_PEER.with(|peer| peer.set(self.0));
    }
}
CREATE_HOOK!(NativePingResponse, { enabled }, NONE, (), (
    owner: *mut c_void, reader: *const c_void, endpoint: *const c_void
), {
    let _scope = ReplyScope::enter();
    CALL_ORIGINAL!(NativePingResponse(owner, reader, endpoint));
});

define_pattern_resolver!(NativeSocketSendTo, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 30 48 8B 5C 24 60 4C 8B F2"
]);
fn reply_destination(
    socket: *mut NativeSocket,
    tracked: *mut NativeSocket,
    original: *const InternetAddr,
    received: *mut InternetAddr,
) -> *const InternetAddr {
    if !tracked.is_null() && socket == tracked && !received.is_null() {
        received
    } else {
        original
    }
}
CREATE_HOOK!(NativeSocketSendTo, { enabled }, NONE, bool, (
    socket: *mut NativeSocket, data: *const u8, size: i32, sent: *mut i32,
    destination: *const InternetAddr
), {
    let received = REPLY_PEER.with(|peer| peer.get());
    let destination = reply_destination(
        socket, PING_SOCKET.load(Ordering::Acquire), destination, received,
    );
    CALL_ORIGINAL!(NativeSocketSendTo(socket, data, size, sent, destination))
});

define_pattern_resolver!(
    NativeSocketClose,
    ["40 53 48 83 EC 20 48 8B D9 48 8B 49 28 48 83 F9 FF 74 19 FF 15"]
);
CREATE_HOOK!(NativeSocketClose, { enabled }, NONE, bool, (socket: *mut NativeSocket), {
    let _ = PING_SOCKET.compare_exchange(
        socket, std::ptr::null_mut(), Ordering::AcqRel, Ordering::Acquire,
    );
    CALL_ORIGINAL!(NativeSocketClose(socket))
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tracked_ping_socket_uses_the_received_address() {
        let ping = 1usize as *mut NativeSocket;
        let other = 2usize as *mut NativeSocket;
        let original = 3usize as *const InternetAddr;
        let received = 4usize as *mut InternetAddr;
        assert_eq!(reply_destination(ping, ping, original, received), received);
        assert_eq!(reply_destination(other, ping, original, received), original);
        assert_eq!(
            reply_destination(ping, ping, original, std::ptr::null_mut()),
            original
        );
        assert_eq!(
            reply_destination(ping, std::ptr::null_mut(), original, received),
            original
        );
    }

    #[test]
    fn the_reply_scope_consumes_the_peer_and_restores_nested_state() {
        let outer = 1usize as *mut InternetAddr;
        let inner = 2usize as *mut InternetAddr;
        RECEIVED_PEER.with(|peer| peer.set(outer));
        {
            let _outer = ReplyScope::enter();
            assert_eq!(REPLY_PEER.with(Cell::get), outer);
            assert!(RECEIVED_PEER.with(Cell::get).is_null());
            RECEIVED_PEER.with(|peer| peer.set(inner));
            {
                let _inner = ReplyScope::enter();
                assert_eq!(REPLY_PEER.with(Cell::get), inner);
            }
            assert_eq!(REPLY_PEER.with(Cell::get), outer);
        }
        assert!(REPLY_PEER.with(Cell::get).is_null());
    }
}
