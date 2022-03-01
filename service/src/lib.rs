use std::any::Any;

use rocket::{Build, Rocket};

mod error;
mod factory;

pub use error::Error;
pub use factory::Factory;

pub trait Service<F>
where
    Self: Any + Send + Sync,
    F: Factory,
{
    fn deploy(&self, factory: &F) -> Deployment;
}

pub enum Deployment {
    Rocket(Rocket<Build>),
}

impl From<Rocket<Build>> for Deployment {
    fn from(r: Rocket<Build>) -> Self {
        Deployment::Rocket(r)
    }
}

#[macro_export]
macro_rules! declare_service {
    ($service_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn $crate::Service<Box<dyn $crate::Factory>> {
            // Ensure constructor returns concrete type.
            let constructor: fn() -> $service_type = $constructor;

            let obj = constructor();
            let boxed: Box<dyn $crate::Service<Box<dyn $crate::Factory>>> = Box::new(obj);
            Box::into_raw(boxed)
        }
    };
}
