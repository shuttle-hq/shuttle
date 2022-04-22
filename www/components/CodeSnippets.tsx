import classnames from "classnames";
import { useState } from "react";
import CodeBlock from "./CodeBlock";

export default function CodeSnippets() {
  const [activeTab, setActiveTab] = useState(1);

  return (
    <div className="mx-auto max-w-6xl px-4 pt-16 pb-20 sm:px-6 lg:px-8 lg:pt-24 lg:pb-28">
      <div className="grid w-full grid-cols-1 gap-4 lg:grid-cols-12">
        <div className="mb-4 lg:col-span-5">
          <h2 className="text-3xl font-extrabold tracking-tight dark:text-gray-200 sm:text-4xl">
            How it works
          </h2>
          <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
            Shuttle is built for Rust.
          </p>

          <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
            A simple cargo command packages up your application, ships it to the
            shuttle build cluster where it's incrementally compiled and
            automatically served on a unique subdomain.
          </p>

          <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
            Shuttle uses simple but powerful annotations to understand your
            dependencies. Infrastructure dependencies like databases or
            key-value stores are spun up for you and everything is automatically
            wired together from the get-go.
          </p>
          <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
            It feels a little magical.
          </p>
        </div>
        <div className="lg:col-span-7">
          <div className="mb-4">
            <div className="sm:hidden">
              <label htmlFor="tabs" className="sr-only">
                Select a tab
              </label>

              <select
                id="tabs"
                name="tabs"
                className="block w-full rounded text-slate-500  dark:bg-gray-600 dark:text-gray-300"
                defaultValue={tabs[activeTab].name}
                onChange={(e) => void setActiveTab(parseInt(e.target.value))}
              >
                {tabs.map((tab, index) => (
                  <option key={index} value={index}>
                    {tab.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="hidden sm:block">
              <nav className="flex space-x-4" aria-label="Tabs">
                {tabs.map((tab, index) => (
                  <button
                    key={index}
                    onClick={() => void setActiveTab(index)}
                    className={classnames(
                      "z-10 cursor-pointer rounded px-3 py-2 text-sm font-medium hover:shadow-md",
                      {
                        "bg-brand-orange2 text-white": activeTab === index,
                        "text-slate-700 hover:bg-slate-200 hover:text-slate-700 dark:text-gray-300 hover:dark:bg-gray-600 hover:dark:text-gray-200":
                          activeTab !== index,
                      }
                    )}
                    aria-current={activeTab === index ? "page" : undefined}
                  >
                    {tab.name}
                  </button>
                ))}
              </nav>
            </div>
          </div>
          <CodeBlock
            language={tabs[activeTab].language}
            showLineNumbers={tabs[activeTab].showLineNumbers}
            code={tabs[activeTab].code}
          />
        </div>
      </div>
    </div>
  );
}

const CARGO_DEPLOYS = `
$ cargo shuttle deploy
   Packaging url-shortener v0.1.0 (/private/shuttle/examples/url-shortener)
   Archiving Cargo.toml
   Archiving Cargo.toml.orig
   Archiving README.md
   Archiving Shuttle.toml
   Archiving migrations/20220324143837_urls.sql
   Archiving src/lib.rs
   Compiling tracing-attributes v0.1.20
   Compiling tokio-util v0.6.9
   Compiling multer v2.0.2
   Compiling hyper v0.14.18
   Compiling rocket_http v0.5.0-rc.1
   Compiling rocket_codegen v0.5.0-rc.1
   Compiling rocket v0.5.0-rc.1
   Compiling shuttle-service v0.2.5
   Compiling url-shortener v0.1.0 (/opt/unveil/crates/s-2)
    Finished dev [unoptimized + debuginfo] target(s) in 1m 01s

        Project:            url-shortener
        Deployment Id:      3d08ac34-ad63-41c1-836b-99afdc90af9f
        Deployment Status:  DEPLOYED
        Host:               url-shortener.shuttleapp.rs
        Created At:         2022-04-01 08:32:34.412602556 UTC
        Database URI:       postgres://***:***@pg.shuttle.rs/db-url-shortener

❯
`.trim();

const USING_SQLX = `
use rocket::{get, routes, Build, Rocket, State};
use shuttle_service::Error;
use sqlx::PgPool;

struct MyState(PgPool);

#[get("/hello")]
fn hello(state: &State<MyState>) -> &'static str {
    // Do things with \`state.0\`...
    "Hello, Postgres!"
}

#[shuttle_service::main]
async fn rocket(
    pool: PgPool
) -> Result<Rocket<Build>, Error> {
    let state = MyState(pool);

    Ok(
        rocket::build()
            .manage(state)
            .mount("/", routes![hello])
    )
}

`.trim();

const HELLO_CLOUD = `
use rocket::{get, routes, Build, Rocket};
use shuttle_service::Error;

#[get("/hello")]
fn hello() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn init() -> Result<Rocket<Build>, Error> {
    Ok(
        rocket::build()
            .mount("/", routes![hello])
    )
}
`.trim();

const USING_AXUM = `
use axum::{routing::get, Router};
use sync_wrapper::SyncWrapper;
use shuttle_service::Error;

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn axum() -> Result<SyncWrapper<Router>, Error> {
    let router = Router::new()
        .route("/hello", get(hello_world));
    let sync_wrapper = SyncWrapper::new(router);

    Ok(sync_wrapper)
}
`.trim();

const tabs = [
  {
    name: "Cargo Deploys",
    code: CARGO_DEPLOYS,
    showLineNumbers: false,
    language: "cargo",
  },
  {
    name: "Hello Cloud",
    code: HELLO_CLOUD,
    showLineNumbers: true,
    language: "rust",
  },
  {
    name: "Using Sqlx",
    code: USING_SQLX,
    showLineNumbers: true,
    language: "rust",
  },
  {
    name: "Using Axum",
    code: USING_AXUM,
    showLineNumbers: true,
    language: "rust",
  },
];
