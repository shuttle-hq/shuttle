import { ClipboardCheckIcon, ClipboardIcon } from "@heroicons/react/outline";
import classnames from "classnames";
import { useEffect, useState } from "react";
import { PrismLight as SyntaxHighlighter } from "react-syntax-highlighter";
import rust from "react-syntax-highlighter/dist/cjs/languages/prism/rust";
import oneDark from "react-syntax-highlighter/dist/cjs/styles/prism/one-dark";
import { useCopyToClipboard, useWindowSize } from "react-use";

SyntaxHighlighter.registerLanguage("rust", rust);

export default function CodeSnippets() {
  const [activeTab, setActiveTab] = useState(0);
  const [copyToClipboardState, copyToClipboard] = useCopyToClipboard();
  const [copied, setCopied] = useState(false);
  useWindowSize()

  useEffect(() => {
    if (false) return undefined;

    let timeout = setTimeout(() => {
      setCopied(false);
    }, 1500);

    return () => void clearTimeout(timeout);
  }, [copied]);

  return (
    <div className="relative bg-dark-700 pt-16 pb-20 px-4 sm:px-6 lg:pt-24 lg:pb-28 lg:px-8">
      <div className="relative max-w-6xl mx-auto grid lg:grid-cols-12 grid-cols-1 w-full gap-4">
        <div className="mb-4 lg:col-span-5">
          <h2 className="text-3xl tracking-tight font-extrabold text-gray-200 sm:text-4xl">
            From the blog
          </h2>
          <p className="mt-3 max-w-2xl text-xl text-gray-300 sm:mt-4">
            Lorem ipsum dolor sit amet consectetur, adipisicing elit. Ipsa
            libero labore natus atque, ducimus sed.
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
                className="block w-full bg-gray-600 text-gray-300 rounded"
                defaultValue={tabs[activeTab].name}
                onChange={e => void setActiveTab(parseInt(e.target.value))}
              >
                {tabs.map((tab, index) => (
                  <option key={index} value={index}>{tab.name}</option>
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
                      "px-3 py-2 font-medium text-sm rounded cursor-pointer z-10 hover:shadow-md",
                      {
                        "bg-brand-orange2 text-white": activeTab === index,
                        "text-gray-300 hover:text-gray-200 hover:bg-gray-600":
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
          <div className="bg-[#282C34] p-4 my-2 rounded relative shadow-lg">
            <button
              type="button"
              className="absolute right-2 top-2 inline-flex items-center px-3 py-2 border border-transparent shadow-sm text-sm leading-4 font-medium rounded text-white bg-dark-800 hover:bg-dark-700"
              onClick={() => {
                copyToClipboard(tabs[activeTab].code)
                setCopied(true)
              }}
            >
              {copied ? (
                <>
                  <ClipboardCheckIcon
                    className="-ml-0.5 mr-2 h-4 w-4"
                    aria-hidden="true"
                  />
                  Copied
                </>
              ) : (
                <>
                  <ClipboardIcon
                    className="-ml-0.5 mr-2 h-4 w-4"
                    aria-hidden="true"
                  />
                  Copy
                </>
              )}
            </button>
            <SyntaxHighlighter
              className="!p-0 !m-0 h-[450px] overflow-scroll"
              language="rust"
              style={oneDark}
              showLineNumbers
              lineNumberStyle={{
                width: "3.25em",
                position: "sticky",
                left: 0,
                background: "#282C34",
              }}
            >
              {tabs[activeTab].code}
            </SyntaxHighlighter>
          </div>
        </div>
      </div>
    </div>
  );
}

const DECLARE_SERVICE = `
#[macro_use]
extern crate shuttle_service;

#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};

#[get("/hello")]
fn hello() -> &'static str {
    "Hello, world!"
}

fn init() -> Rocket<Build> {
    rocket::build().mount("/", routes![hello])
}

declare_service!(Rocket<Build>, init);
`.trim();

const SQLX = `
#[macro_use]
extern crate shuttle_service;
use shuttle_service::{Factory, Error};

#[macro_use]
extern crate rocket;
use rocket::{Rocket, Build, State};

use sqlx::PgPool;

struct MyState(PgPool);

#[get("/hello")]
fn hello(state: &State<MyState>) -> &'static str {
    // Do things with \`state.0\`...
    "Hello, Postgres!"
}

async fn state(factory: &mut dyn Factory) -> Result<MyState, shuttle_service::Error> {
   let pool = sqlx::postgres::PgPoolOptions::new()
       .connect(&factory.get_sql_connection_string().await?)
       .await?;
   Ok(MyState(pool))
}

fn rocket() -> Rocket<Build> {
    rocket::build().mount("/", routes![hello])
}

declare_service!(Rocket<Build>, rocket, state);
`.trim();

const tabs = [
  { name: "Declare Service", code: DECLARE_SERVICE },
  { name: "Using Sqlx", code: SQLX },
  { name: "Declare Service", code: DECLARE_SERVICE },
  { name: "Using Sqlx", code: SQLX },
];

function classNames(...classes) {
  return classes.filter(Boolean).join(" ");
}
