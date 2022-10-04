use shuttle_service::{Error, Service};

#[derive(Clone)]
pub struct MyService;

#[shuttle_service::async_trait]
impl Service for MyService {
    async fn bind(
        mut self: Box<Self>,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        println!("service is binding");
        Ok(())
    }
}

#[shuttle_service::main]
async fn shuttle() -> Result<MyService, Error> {
    Ok(MyService {})
}

// async fn __shuttle_wrapper(
//     _factory: &mut dyn shuttle_service::Factory,
//     runtime: &shuttle_service::Runtime,
//     logger: Box<dyn shuttle_service::log::Log>,
// ) -> Result<Box<dyn Service>, Error> {
//     runtime
//         .spawn_blocking(move || {
//             shuttle_service::log::set_boxed_logger(logger)
//                 .map(|()| {
//                     shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info)
//                 })
//                 .expect("logger set should succeed");
//         })
//         .await
//         .map_err(|e| {
//             if e.is_panic() {
//                 let mes = e
//                     .into_panic()
//                     .downcast_ref::<&str>()
//                     .map(|x| x.to_string())
//                     .unwrap_or_else(|| "<no panic message>".to_string());

//                 shuttle_service::Error::BuildPanic(mes)
//             } else {
//                 shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
//             }
//         })?;

//     runtime
//         .spawn(async {
//             shuttle()
//                 .await
//                 .map(|ok| Box::new(ok) as Box<dyn shuttle_service::Service>)
//         })
//         .await
//         .map_err(|e| {
//             if e.is_panic() {
//                 let mes = e
//                     .into_panic()
//                     .downcast_ref::<&str>()
//                     .map(|x| x.to_string())
//                     .unwrap_or_else(|| "<no panic message>".to_string());

//                 shuttle_service::Error::BuildPanic(mes)
//             } else {
//                 shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
//             }
//         })?
// }

// fn __binder(
//     service: Box<dyn shuttle_service::Service>,
//     addr: std::net::SocketAddr,
//     runtime: &shuttle_service::Runtime,
// ) -> shuttle_service::ServeHandle {
//     runtime.spawn(async move { service.bind(addr).await })
// }

// #[no_mangle]
// pub extern "C" fn _create_service() -> *mut shuttle_service::Bootstrapper {
//     let builder: shuttle_service::StateBuilder<Box<dyn shuttle_service::Service>> =
//         |factory, runtime, logger| Box::pin(__shuttle_wrapper(factory, runtime, logger));

//     let bootstrapper = shuttle_service::Bootstrapper::new(
//         builder,
//         __binder,
//         shuttle_service::Runtime::new().unwrap(),
//     );

//     let boxed = Box::new(bootstrapper);
//     Box::into_raw(boxed)
// }
