use std::any::Any;
use std::cell::{Ref, RefCell};

use container_of::container_of;

#[repr(C)]
pub struct UbusContainer<T: Default> {
    inner: T,
    ctxt: ContextContainer,
}

impl<T: Default> UbusContainer<T> {
    pub fn new(ctxt_value: impl Any + 'static) -> Self {
        Self {
            inner: T::default(),
            ctxt: ContextContainer::new(ctxt_value),
        }
    }

    pub fn from_ptr<'a>(ptr: *mut T) -> &'a Self {
        unsafe {
            let ubus_container = container_of!(ptr, Self, inner);
            &*(ubus_container as *const Self)
        }
    }

    pub(crate) fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn context(&self) -> &ContextContainer {
        &self.ctxt
    }

    pub fn context_mut(&mut self) -> &mut ContextContainer {
        &mut self.ctxt
    }
}

pub struct ContextContainer {
    inner: RefCell<Box<dyn Any>>,
}

impl ContextContainer {
    pub fn new<T: 'static>(value: T) -> Self {
        Self {
            inner: RefCell::new(Box::new(value)),
        }
    }

    pub fn get<T: 'static>(&self) -> Option<Ref<T>> {
        let borrowed = self.inner.borrow();
        if borrowed.is::<T>() {
            Some(Ref::map(borrowed, |any| {
                any.downcast_ref::<T>().expect("Type mismatch")
            }))
        } else {
            None
        }
    }

    pub fn get_mut<T: 'static>(&self) -> Option<std::cell::RefMut<T>> {
        let borrowed_mut = self.inner.borrow_mut();
        if borrowed_mut.is::<T>() {
            Some(std::cell::RefMut::map(borrowed_mut, |any| {
                any.downcast_mut::<T>().expect("Type mismatch")
            }))
        } else {
            None
        }
    }
}
