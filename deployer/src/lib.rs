use dal::Sqlite;
use docker::context::ContextProvider;
use driver::{project::TaskRouter, task::BoxedTask};

pub mod account;
pub mod api;
pub mod args;
pub mod dal;
pub mod docker;
pub mod driver;
pub mod error;
pub mod handlers;
pub mod manager;
pub mod project;

pub struct DeployerService {
    task_router: TaskRouter<BoxedTask>,
    context: ContextProvider,
    sqlite: Sqlite,
}

impl DeployerService {
    pub fn task_router(&self) -> TaskRouter<BoxedTask> {
        self.task_router.clone()
    }

    pub fn context(&self) -> &ContextProvider {
        &self.context
    }

    pub fn sqlite(&self) -> &Sqlite {
        &self.sqlite
    }
}
