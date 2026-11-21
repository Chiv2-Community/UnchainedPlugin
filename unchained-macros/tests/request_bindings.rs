#![feature(arbitrary_self_types_pointers)]

use std::ffi::c_void;
use unchained_macros::bind_resolved_request_function;

struct OpenAPIClientAPI {
    value: i32,
}

impl OpenAPIClientAPI {
    #[bind_resolved_request_function(GetPost_GetMotd)]
    pub unsafe fn get_post_get_motd(
        self: *mut Self,
        result: *mut i32,
        request: *const i32,
        delegate: *mut c_void,
    ) -> *mut i32 {}

    #[bind_resolved_request_function(GetPost_Borrow)]
    unsafe fn borrow<'a>(&self, result: &'a mut i32) -> &'a mut i32 {}

    #[bind_resolved_request_function(GetPost_Missing)]
    unsafe fn missing(&self) {}
}

unsafe extern "C" fn get_motd(
    this: *mut OpenAPIClientAPI,
    result: *mut i32,
    request: *const i32,
    delegate: *mut c_void,
) -> *mut i32 {
    assert!(delegate.is_null());
    unsafe { *result = (*this).value + *request; }
    result
}

unsafe extern "C" fn borrow<'a>(this: &OpenAPIClientAPI, result: &'a mut i32) -> &'a mut i32 {
    *result = this.value;
    result
}

#[test]
fn request_keys_bind_without_corresponding_resolver_types() {
    for registration in inventory::iter::<unchained_macros::FunctionBindingRegistration> {
        let binding = registration.binding;
        let address = match binding.name() {
            "GetPost_GetMotd" => get_motd as *const () as usize,
            "GetPost_Borrow" => borrow as *const () as usize,
            "GetPost_Missing" => continue,
            name => panic!("Unexpected request binding {name}"),
        };
        unsafe { binding.initialize(1, (address - 1) as u64).unwrap(); }
    }
    let mut client = OpenAPIClientAPI { value: 40 };
    let mut output = 0;
    let output_ptr = &raw mut output;
    let request = 2;
    let client_ptr = &raw mut client;
    unsafe {
        assert_eq!(client_ptr.get_post_get_motd(output_ptr, &request, std::ptr::null_mut()), output_ptr);
        assert_eq!(output, 42);
        assert_eq!(client.borrow(&mut output) as *mut i32, output_ptr);
        assert_eq!(output, 40);
    }
}

#[test]
#[should_panic(expected = "Native binding GetPost_Missing is not initialized")]
fn missing_request_binding_fails_before_native_call() {
    unsafe { OpenAPIClientAPI { value: 0 }.missing(); }
}
