use serde::{Deserialize, Serialize};
use shuttle_service::error::CustomError;
use sqlx::{Executor, FromRow, PgPool};
use thruster::{
    context::{
        context_ext::ContextExt, hyper_request::HyperRequest,
        typed_hyper_context::TypedHyperContext,
    },
    errors::{ErrorSet, ThrusterError},
    m, middleware_fn, App, Context, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
};

type Ctx = TypedHyperContext<RequestConfig>;

#[derive(Deserialize)]
struct TodoNew {
    pub note: String,
}

#[derive(Serialize, FromRow)]
struct Todo {
    pub id: i32,
    pub note: String,
}

struct ServerConfig {
    pool: PgPool,
}

#[derive(Clone)]
struct RequestConfig {
    pool: PgPool,
}

fn generate_context(request: HyperRequest, state: &ServerConfig, _path: &str) -> Ctx {
    Ctx::new(
        request,
        RequestConfig {
            pool: state.pool.clone(),
        },
    )
}

#[middleware_fn]
async fn retrieve(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    let id: i32 = context
        .params()
        .get("id")
        .ok_or_else(|| {
            ThrusterError::parsing_error(
                Ctx::new_without_request(context.extra.clone()),
                "id is required",
            )
        })?
        .param
        .parse()
        .map_err(|_e| {
            ThrusterError::parsing_error(
                Ctx::new_without_request(context.extra.clone()),
                "id must be a number",
            )
        })?;

    let todo: Todo = sqlx::query_as("SELECT * FROM todos WHERE id = $1")
        .bind(id)
        .fetch_one(&context.extra.pool)
        .await
        .map_err(|_e| {
            ThrusterError::not_found_error(Ctx::new_without_request(context.extra.clone()))
        })?;

    context.set_body(serde_json::to_vec(&todo).unwrap());

    Ok(context)
}

#[middleware_fn]
async fn add(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
    let extra = context.extra.clone();

    let todo_req = context
        .get_json::<TodoNew>()
        .await
        .map_err(|_e| ThrusterError::generic_error(Ctx::new_without_request(extra)))?;

    let todo: Todo = sqlx::query_as("INSERT INTO todos(note) VALUES ($1) RETURNING id, note")
        .bind(&todo_req.note)
        .fetch_one(&context.extra.pool)
        .await
        .map_err(|_e| {
            ThrusterError::generic_error(Ctx::new_without_request(context.extra.clone()))
        })?;

    context.set_body(serde_json::to_vec(&todo).unwrap());

    Ok(context)
}

#[shuttle_service::main]
async fn thruster(
    #[shuttle_aws_rds::Postgres] pool: PgPool,
) -> shuttle_service::ShuttleThruster<HyperServer<Ctx, ServerConfig>> {
    pool.execute(include_str!("../schema.sql"))
        .await
        .map_err(CustomError::new)?;

    Ok(HyperServer::new(
        App::<HyperRequest, Ctx, ServerConfig>::create(generate_context, ServerConfig { pool })
            .post("/todos", m![add])
            .get("/todos/:id", m![retrieve]),
    ))
}
