#[macro_export]
macro_rules! static_ubus_object {
    ($internal_name:ident, $object_name:expr, [$(($method_name:expr, $callback:expr)),*]) => {
        lazy_static::lazy_static! {
            static ref __METHODS_VEC: Vec<ubus_method> = vec![
                $(
                    ubus_method::new_const(
                        $method_name.as_ptr() ,
                        $callback,
                    )
                ),*
            ];

            static ref __METHODS: &'static [ubus_method] = &__METHODS_VEC[..];

            static ref __TYPE: ubus_object_type = ubus_object_type::new_const(
                $object_name.as_ptr() ,
                *__METHODS,
            );

            pub static ref $internal_name: ubus_object = ubus_object::new_const(
                 $object_name.as_ptr() ,
                 (&*__TYPE as *const ubus_object_type) as *mut ubus_object_type ,
                *__METHODS,
            );
        }
    };
}
