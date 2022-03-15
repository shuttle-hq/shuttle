use shuttle_service::Service;

// A wrapper to catch all panics from services created by client code
pub(crate) struct PanicSafeService {
    service: Box<dyn Service>,
}

impl PanicSafeService {
    pub fn new(service: Box<dyn Service>) -> Self {
        Self { service }
    }
}

impl Service for PanicSafeService {
    fn bind(&mut self, addr: std::net::SocketAddr) -> Result<(), shuttle_service::Error> {
        self.service.bind(addr)
    }
    fn build(
        &mut self,
        factory: &mut dyn shuttle_service::Factory,
    ) -> Result<(), shuttle_service::Error> {
        self.service.build(factory)
    }
}
