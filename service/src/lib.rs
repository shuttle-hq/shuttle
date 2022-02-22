use std::any::Any;

pub trait Service: Any + Send + Sync {
    fn start(&self) -> &'static str;
}

#[macro_export]
macro_rules!  declare_service {
    ($service_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            let obj = constructor();
            let boxed: Box<dyn $crate::Service> = Box::new(obj);
            Box::into_raw(boxed)
        }
    }
}
