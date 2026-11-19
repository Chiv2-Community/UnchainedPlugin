//! UE4.25 Windows socket ABI used by the native ping hook.
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::ue::{EFindName, FName, FString};

#[repr(C)]
pub struct NativeSocket {
    pub vtable: *const usize,
}

#[repr(C)]
pub struct InternetAddr {
    pub vtable: *const usize,
}

#[repr(C)]
pub struct SocketSubsystem {
    pub vtable: *const usize,
}

/// FUdpSocketBuilder's existing layout; the hook never changes its fields.
#[repr(C)]
pub struct UdpSocketBuilder {
    pub broadcast: bool,
    pub blocking: bool,
    pub bound: bool,
    _padding: u8,
    pub ipv4_address: u32,
    pub port: u16,
    _endpoint_padding: [u8; 6],
    pub description: FString,
    multicast_interface: u32,
    _interface_padding: u32,
    multicast_groups: [usize; 2],
    multicast_loopback: bool,
    multicast_ttl: u8,
    _multicast_padding: [u8; 2],
    pub receive_buffer: i32,
    pub reusable: bool,
    _reusable_padding: [u8; 3],
    pub send_buffer: i32,
}

#[repr(C)]
#[derive(Default)]
struct SharedAddr {
    address: *mut InternetAddr,
    controller: *mut c_void,
}

/// The observed Windows vtables use eight-byte slots.
unsafe fn method<F: Copy>(object: *mut c_void, offset: usize) -> F {
    assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<usize>());
    let table = *(object as *const *const usize);
    let entry = *table.add(offset / 8);
    std::mem::transmute_copy(&entry)
}

impl Drop for SharedAddr {
    fn drop(&mut self) {
        if self.controller.is_null() {
            return;
        }
        unsafe {
            // FReferenceControllerBase: vtable, shared count, weak count.
            let shared = &*(self.controller.byte_add(8) as *const AtomicI32);
            if shared.fetch_sub(1, Ordering::AcqRel) == 1 {
                let destroy: unsafe extern "C" fn(*mut c_void) = method(self.controller, 0);
                destroy(self.controller);
                let weak = &*(self.controller.byte_add(12) as *const AtomicI32);
                if weak.fetch_sub(1, Ordering::AcqRel) == 1 {
                    let delete: unsafe extern "C" fn(*mut c_void, u32) = method(self.controller, 8);
                    delete(self.controller, 1);
                }
            }
        }
    }
}

pub unsafe fn name(text: &[u16]) -> FName {
    let mut result = FName::default();
    crate::ue::fname_ctor_wchar(&mut result, text.as_ptr(), EFindName::Add)
        .expect("FName constructor is initialized before socket creation");
    result
}

pub unsafe fn destroy_socket(subsystem: *mut SocketSubsystem, socket: *mut NativeSocket) {
    let destroy: unsafe extern "C" fn(*mut SocketSubsystem, *mut NativeSocket) =
        method(subsystem.cast(), 0x38);
    destroy(subsystem, socket);
}

/// Create a real UE socket, retaining UE's receiver and packet handler.
///
/// # Safety
/// Pointers must refer to the verified UE4.25 Windows implementations.
pub unsafe fn build_ipv6_ping(
    subsystem: *mut SocketSubsystem,
    builder: &UdpSocketBuilder,
) -> Option<*mut NativeSocket> {
    if subsystem.is_null()
        || !builder.bound
        || builder.port == 0
        || builder.multicast_groups[1] != 0
    {
        return None;
    }
    let datagram = name(&[68, 71, 114, 97, 109, 0]);
    let ipv6 = name(&[73, 80, 118, 54, 0]);
    let create: unsafe extern "C" fn(
        *mut SocketSubsystem,
        *const FName,
        *const FString,
        *const FName,
    ) -> *mut NativeSocket = method(subsystem.cast(), 0x18);
    let socket = create(subsystem, &datagram, &builder.description, &ipv6);
    if socket.is_null() {
        return None;
    }
    if configure_socket(subsystem, socket, builder) {
        Some(socket)
    } else {
        destroy_socket(subsystem, socket);
        None
    }
}

unsafe fn configure_socket(
    subsystem: *mut SocketSubsystem,
    socket: *mut NativeSocket,
    builder: &UdpSocketBuilder,
) -> bool {
    // FSocketBSD::Socket at 0x28 is verified by its RecvFrom/SendTo/Close bodies.
    #[cfg_attr(windows, link(name = "ws2_32"))]
    extern "system" {
        fn setsockopt(socket: usize, level: i32, option: i32, value: *const u8, len: i32) -> i32;
    }
    let handle = *socket.cast::<u8>().add(0x28).cast::<usize>();
    let dual_stack = 0i32;
    if setsockopt(handle, 41, 27, (&dual_stack as *const i32).cast(), 4) != 0 {
        return false;
    }
    for (offset, enabled) in [
        (0x98, !builder.blocking),
        (0xe8, builder.reusable),
        (0xa0, builder.broadcast),
    ] {
        let set: unsafe extern "C" fn(*mut NativeSocket, bool) -> bool =
            method(socket.cast(), offset);
        if !set(socket, enabled) {
            return false;
        }
    }
    let mut address = SharedAddr::default();
    let create_address: unsafe extern "C" fn(
        *mut SocketSubsystem,
        *mut SharedAddr,
    ) -> *mut SharedAddr = method(subsystem.cast(), 0x90);
    create_address(subsystem, &mut address);
    if address.address.is_null() {
        return false;
    }
    let set_ip: unsafe extern "C" fn(*mut InternetAddr, *const u16, *mut bool) =
        method(address.address.cast(), 0x10);
    let mut valid = false;
    set_ip(address.address, [58u16, 58, 0].as_ptr(), &mut valid);
    if !valid {
        return false;
    }
    let set_port: unsafe extern "C" fn(*mut InternetAddr, i32) =
        method(address.address.cast(), 0x28);
    set_port(address.address, i32::from(builder.port));
    let bind: unsafe extern "C" fn(*mut NativeSocket, *const InternetAddr) -> bool =
        method(socket.cast(), 0x18);
    if !bind(socket, address.address) {
        return false;
    }
    for (offset, size) in [
        (0x108, builder.receive_buffer),
        (0x100, builder.send_buffer),
    ] {
        if size > 0 {
            let set: unsafe extern "C" fn(*mut NativeSocket, i32, *mut i32) -> bool =
                method(socket.cast(), offset);
            let mut actual = 0;
            if !set(socket, size, &mut actual) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn builder_offsets_match_the_native_windows_code() {
        assert_eq!(std::mem::offset_of!(UdpSocketBuilder, port), 8);
        assert_eq!(std::mem::offset_of!(UdpSocketBuilder, description), 0x10);
        assert_eq!(std::mem::offset_of!(UdpSocketBuilder, receive_buffer), 0x3c);
        assert_eq!(std::mem::offset_of!(UdpSocketBuilder, reusable), 0x40);
        assert_eq!(std::mem::offset_of!(UdpSocketBuilder, send_buffer), 0x44);
        assert_eq!(std::mem::size_of::<UdpSocketBuilder>(), 0x48);
        assert_eq!(std::mem::size_of::<SharedAddr>(), 16);
    }

    #[repr(C)]
    struct Controller {
        vtable: *const usize,
        shared: AtomicI32,
        weak: AtomicI32,
        destroyed: AtomicBool,
        deleted: AtomicBool,
    }

    unsafe extern "C" fn destroy(controller: *mut c_void) {
        (*(controller as *mut Controller))
            .destroyed
            .store(true, Ordering::Relaxed);
    }

    unsafe extern "C" fn delete(controller: *mut c_void, flag: u32) {
        assert_eq!(flag, 1);
        (*(controller as *mut Controller))
            .deleted
            .store(true, Ordering::Relaxed);
    }

    #[test]
    fn native_shared_address_releases_only_the_final_reference() {
        let vtable = [destroy as *const () as usize, delete as *const () as usize];
        let mut controller = Controller {
            vtable: vtable.as_ptr(),
            shared: AtomicI32::new(2),
            weak: AtomicI32::new(1),
            destroyed: AtomicBool::new(false),
            deleted: AtomicBool::new(false),
        };
        let pointer = (&mut controller as *mut Controller).cast();
        drop(SharedAddr {
            address: std::ptr::null_mut(),
            controller: pointer,
        });
        assert!(!controller.destroyed.load(Ordering::Relaxed));
        assert!(!controller.deleted.load(Ordering::Relaxed));
        drop(SharedAddr {
            address: std::ptr::null_mut(),
            controller: pointer,
        });
        assert!(controller.destroyed.load(Ordering::Relaxed));
        assert!(controller.deleted.load(Ordering::Relaxed));
        assert_eq!(controller.shared.load(Ordering::Relaxed), 0);
        assert_eq!(controller.weak.load(Ordering::Relaxed), 0);
    }
}
