#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

mod libubus_bindgen;

use std::os::raw::{c_char, c_int};

pub use libubus_bindgen::{
    blob_attr, ubus_add_object, ubus_connect, ubus_context, ubus_event_handler,
    ubus_event_handler_t, ubus_free, ubus_handler_t, ubus_method, ubus_object, ubus_object_type,
    ubus_reconnect, ubus_register_event_handler, ubus_remove_object, ubus_request_data,
};

use libubus_bindgen::{avl_node, blob_buf, blob_buf_free, list_head, uloop_fd};

unsafe impl Sync for ubus_method {}
unsafe impl Sync for ubus_object_type {}
unsafe impl Sync for ubus_object {}

impl ubus_method {
    pub const fn new_const(name: *const c_char, handler: ubus_handler_t) -> Self {
        ubus_method {
            name,
            handler,
            mask: 0,
            tags: 0,
            policy: std::ptr::null(),
            n_policy: 0,
        }
    }
}

impl ubus_object_type {
    pub const fn new_const(name: *const c_char, methods: &[ubus_method]) -> Self {
        ubus_object_type {
            name,
            methods: methods.as_ptr() as *const ubus_method,
            n_methods: methods.len() as i32,
            id: 0,
        }
    }
}

impl ubus_object {
    pub const fn new_const(
        name: *const c_char,
        type_: *mut ubus_object_type,
        methods: &[ubus_method],
    ) -> Self {
        ubus_object {
            name,
            type_,
            methods: methods.as_ptr() as *const ubus_method,
            n_methods: methods.len() as i32,
            avl: avl_node {
                list: list_head {
                    next: std::ptr::null_mut(),
                    prev: std::ptr::null_mut(),
                },
                parent: std::ptr::null_mut(),
                left: std::ptr::null_mut(),
                right: std::ptr::null_mut(),
                key: std::ptr::null(),
                balance: 0,
                leader: false,
            },
            id: 0,
            path: std::ptr::null(),
            subscribe_cb: None,
            has_subscribers: false,
        }
    }
}

pub unsafe fn ubus_unregister_event_handler(
    ctx: *mut ubus_context,
    ev: *mut ubus_event_handler,
) -> c_int {
    ubus_remove_object(ctx, &mut (*ev).obj as *mut ubus_object)
}

pub unsafe fn ubus_handle_event(ctx: *mut ubus_context) {
    let sock = &mut (*ctx).sock;
    let cb = sock.cb.unwrap();
    cb(sock as *mut uloop_fd, libubus_bindgen::ULOOP_READ);
}

impl Drop for blob_buf {
    fn drop(&mut self) {
        unsafe { blob_buf_free(self as *mut blob_buf) };
    }
}
