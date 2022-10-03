use mongodb::bson::doc;
use mongodb::bson::oid::ObjectId;
use mongodb::{Collection, Database};
use poem::{
    error::{BadRequest, NotFoundError},
    get, handler,
    middleware::AddData,
    post,
    web::{Data, Json},
    EndpointExt, FromRequest, Request, RequestBody, Result, Route,
};
use serde::{Deserialize, Serialize};

struct ObjectIdGuard(ObjectId);

#[poem::async_trait]
impl<'a> FromRequest<'a> for ObjectIdGuard {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let id = req.path_params::<String>()?;
        let obj_id = ObjectId::parse_str(id).map_err(BadRequest)?;
        Ok(ObjectIdGuard(obj_id))
    }
}

#[handler]
async fn retrieve(
    ObjectIdGuard(id): ObjectIdGuard,
    collection: Data<&Collection<Todo>>,
) -> Result<Json<serde_json::Value>> {
    let filter = doc! {"_id": id};
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
    #[shuttle_shared_db::MongoDb] db: Database,
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
