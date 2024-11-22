use core::slice;

use std::any::Any;

use std::os::raw::{c_char, c_int};
use std::os::unix::io::{AsRawFd, RawFd};
use std::{fmt, io};

use std::{collections::HashMap, sync::Mutex};
use std::{marker::PhantomPinned, pin::Pin};

use lazy_static::lazy_static;

use mio::{event::Source, unix::SourceFd};
use mio::{Interest, Registry, Token};

use ubus::{Blob, BlobIter, BlobMsg, BlobTag};

use crate::libubus::{
    blob_attr, ubus_add_object, ubus_connect, ubus_context, ubus_event_handler,
    ubus_event_handler_t, ubus_free, ubus_handle_event, ubus_object, ubus_reconnect,
    ubus_register_event_handler, ubus_remove_object, ubus_unregister_event_handler,
};

use crate::container::UbusContainer;

lazy_static! {
    static ref UBUS_CONTEXT_REGISTRY: Mutex<HashMap<UbusContextMutPtr, UbusServerMutPtr>> =
        Mutex::new(HashMap::new());
}

pub struct UbusServer {
    fd: c_int,
    active: bool,
    ubus_ctx: *mut ubus_context,
    ubus_objs: Vec<UbusContainer<ubus_object>>,
    events: Vec<UbusContainer<ubus_event_handler>>,
    _pin: PhantomPinned,
}

struct UbusServerMutPtr(*mut UbusServer);
#[derive(Eq, Hash, PartialEq)]
struct UbusContextMutPtr(*mut ubus_context);

unsafe impl Send for UbusServerMutPtr {}
unsafe impl Send for UbusContextMutPtr {}

impl UbusServer {
    pub fn add_object_static(
        self: Pin<&mut Self>,
        obj: &'static ubus_object,
        ctxt: impl Any + 'static,
    ) -> io::Result<()> {
        let self_mut = unsafe { self.get_unchecked_mut() };

        self_mut.ubus_objs.push(UbusContainer::new(ctxt));

        let ubus_obj_wrp = self_mut.ubus_objs.last_mut().unwrap();
        let ubus_obj = ubus_obj_wrp.get_mut();
        ubus_obj.name = obj.name;
        ubus_obj.type_ = obj.type_;
        ubus_obj.methods = obj.methods;
        ubus_obj.n_methods = obj.n_methods;

        let ret = unsafe { ubus_add_object(self_mut.ubus_ctx, ubus_obj as *mut ubus_object) };
        if ret != 0 {
            let _ = self_mut.ubus_objs.pop();

            return Err(io::Error::new(io::ErrorKind::Other, ret.to_string()));
        }

        Ok(())
    }

    pub fn register_event_handler(
        self: Pin<&mut Self>,
        event_handler: ubus_event_handler_t,
        pattern: *const c_char,
        ctxt: impl Any + 'static,
    ) -> io::Result<()> {
        let self_mut = unsafe { self.get_unchecked_mut() };

        self_mut.events.push(UbusContainer::new(ctxt));

        let ubus_event_wrp = self_mut.events.last_mut().unwrap();
        let ubus_event = ubus_event_wrp.get_mut();
        ubus_event.cb = event_handler;

        let ret = unsafe {
            ubus_register_event_handler(
                self_mut.ubus_ctx,
                ubus_event as *mut ubus_event_handler,
                pattern,
            )
        };
        if ret != 0 {
            let _ = self_mut.events.pop();

            return Err(io::Error::new(io::ErrorKind::Other, ret.to_string()));
        }

        Ok(())
    }

    pub fn handle_event(self: Pin<&Self>) {
        let self_ref = self.get_ref();
        unsafe { ubus_handle_event(self_ref.ubus_ctx) };
    }

    pub fn is_online(self: Pin<&Self>) -> bool {
        self.active
    }

    pub fn set_connection_lost(self: Pin<&mut Self>, cl: bool) {
        let self_mut = unsafe { self.get_unchecked_mut() };
        let ubus_ctx = unsafe { &mut *(self_mut.ubus_ctx as *mut ubus_context) };

        ubus_ctx.connection_lost = if cl {
            Some(UbusServer::connection_lost_cb)
        } else {
            None
        };
    }

    pub fn reconnect(self: Pin<&mut Self>) -> io::Result<()> {
        let self_mut = unsafe { self.get_unchecked_mut() };
        let ubus_ctx = unsafe { &*(self_mut.ubus_ctx as *const ubus_context) };

        if ubus_ctx.connection_lost.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "connection_lost is not enabled",
            ));
        }

        let ret = unsafe { ubus_reconnect(self_mut.ubus_ctx, std::ptr::null()) };
        if ret != 0 {
            return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
        }

        self_mut.active = true;

        Ok(())
    }

    unsafe extern "C" fn connection_lost_cb(ubus_ctx: *mut ubus_context) {
        let ubus_ctx_map = UBUS_CONTEXT_REGISTRY.lock().unwrap();
        if let Some(ubus_server_raw) = ubus_ctx_map.get(&UbusContextMutPtr(ubus_ctx)) {
            let ubus_server = &mut *ubus_server_raw.0;

            ubus_server.active = false;
        }
    }

    pub fn parse_msg(self: Pin<&mut Self>, msg: *mut blob_attr) -> BlobIter<Blob> {
        assert_eq!(std::mem::size_of::<u32>(), BlobTag::SIZE);

        let tag_bytes = unsafe {
            let value = std::ptr::read(msg as *const u32) as u32;
            value.to_ne_bytes()
        };
        let tag = BlobTag::from_bytes(tag_bytes);
        let data =
            unsafe { slice::from_raw_parts((*msg).data.as_ptr() as *const u8, tag.inner_len()) };

        BlobIter::<Blob>::new(data)
    }

    pub fn parse_msg_cb(
        self: Pin<&mut Self>,
        msg: *mut blob_attr,
        mut on_msg: impl FnMut(BlobMsg),
    ) {
        for attr in self.parse_msg(msg) {
            let Ok(blob_msg) = attr.try_into() else {
                return;
            };
            on_msg(blob_msg);
        }
    }

    pub fn from_ubus_ctx<'a>(ubus_ctx: *mut ubus_context) -> Pin<&'a mut UbusServer> {
        let ubus_ctx_map = UBUS_CONTEXT_REGISTRY.lock().unwrap();
        let ubus_server =
            if let Some(ubus_server_raw) = ubus_ctx_map.get(&UbusContextMutPtr(ubus_ctx)) {
                unsafe { &mut *ubus_server_raw.0 }
            } else {
                panic!();
            };

        unsafe { Pin::new_unchecked(ubus_server) }
    }

    pub fn new() -> io::Result<Pin<Box<Self>>> {
        let ubus_ctx = unsafe { ubus_connect(std::ptr::null()) };
        if ubus_ctx.is_null() {
            return Err(io::Error::from(io::ErrorKind::ConnectionAborted));
        }

        let mut ubus_server = Box::pin(UbusServer {
            fd: unsafe { (*ubus_ctx).sock.fd },
            active: true,
            ubus_ctx,
            ubus_objs: Vec::new(),
            events: Vec::new(),
            _pin: PhantomPinned,
        });
        let ubus_server_mut = unsafe { ubus_server.as_mut().get_unchecked_mut() };

        UBUS_CONTEXT_REGISTRY.lock().unwrap().insert(
            UbusContextMutPtr(ubus_server_mut.ubus_ctx),
            UbusServerMutPtr(ubus_server_mut as *mut UbusServer),
        );

        Ok(ubus_server)
    }
}

impl Drop for UbusServer {
    fn drop(&mut self) {
        fn inner_drop(mut this: Pin<&mut UbusServer>) {
            let ubus_ctx = this.as_mut().ubus_ctx;

            let ubus_server = unsafe { this.as_mut().get_unchecked_mut() };
            for ubus_obj in ubus_server.ubus_objs.iter_mut() {
                unsafe { ubus_remove_object(ubus_ctx, ubus_obj.get_mut() as *mut ubus_object) };
            }
            for ubus_event in ubus_server.events.iter_mut() {
                unsafe {
                    ubus_unregister_event_handler(
                        ubus_ctx,
                        ubus_event.get_mut() as *mut ubus_event_handler,
                    )
                };
            }

            UBUS_CONTEXT_REGISTRY
                .lock()
                .unwrap()
                .remove(&UbusContextMutPtr(ubus_ctx));

            unsafe { ubus_free(ubus_ctx) };
        }

        inner_drop(unsafe { Pin::new_unchecked(self) });
    }
}

impl AsRawFd for UbusServer {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Source for UbusServer {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.fd).register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.fd).reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        SourceFd(&self.fd).deregister(registry)
    }
}

impl fmt::Display for UbusServer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "UbusServer{{ fd={}; ubus_ctx={:p}; }}",
            self.fd, self.ubus_ctx,
        )
    }
}
