pub struct Template {
    pub name: &'static str,
    pub url: &'static str,
}

pub fn templates() -> &'static [Template] {
    &[
        Template {
            name: "Backup Microservice",
            url: "https://github.com/mvish77/backup-microservice",
        },
        Template {
            name: "ToDo App (NextJS Fullstack)",
            url: "https://github.com/hoangsonww/ToDo-App-NextJS-Fullstack",
        },
        Template {
            name: "FastAPI Fullstack",
            url: "https://github.com/fastapi/full-stack-fastapi-template",
        },
    ]
}
