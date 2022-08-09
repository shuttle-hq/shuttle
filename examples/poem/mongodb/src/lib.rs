use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::{Collection, Database};
use poem::{
    error::{BadRequest, NotFoundError},
    get, handler,
    middleware::AddData,
    post,
    web::{Data, Json, Path},
    EndpointExt, Result, Route,
};
use serde::{Deserialize, Serialize};

#[handler]
async fn retrieve(
    Path(id): Path<String>,
    collection: Data<&Collection<Todo>>,
) -> Result<Json<serde_json::Value>> {
    let filter = doc! {"_id": ObjectId::parse_str(id).map_err(BadRequest)?};
    let todo = collection
        .find_one(filter, None)
        .await
        .map_err(BadRequest)?;

    match todo {
        Some(todo) => Ok(Json(serde_json::json!(todo))),
        None => Err(NotFoundError.into()),
    }
}

#[handler]
async fn add(Json(todo): Json<Todo>, collection: Data<&Collection<Todo>>) -> Result<String> {
    let todo_id = collection
        .insert_one(todo, None)
        .await
        .map_err(BadRequest)?;

    Ok(todo_id
        .inserted_id
        .as_object_id()
        .expect("id is objectId")
        .to_string())
}

#[shuttle_service::main]
async fn main(
    #[shared::MongoDb] db: Database,
) -> shuttle_service::ShuttlePoem<impl poem::Endpoint> {
    let collection = db.collection::<Todo>("todos");

    let app = Route::new()
        .at("/todo", post(add))
        .at("/todo/:id", get(retrieve))
        .with(AddData::new(collection));

    Ok(app)
}

#[derive(Debug, Serialize, Deserialize)]
struct Todo {
    pub note: String,
}
