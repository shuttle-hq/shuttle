---
alwaysApply: true
---

# Shuttle Development Rules

## Core Setup

Always use `#[shuttle_runtime::main]` as your entry point. Can be other frameworks too, you can use the Shuttle MCP - search docs tool to find the correct code for your framework.

```rust
#[shuttle_runtime::main]
async fn main() -> ShuttleAxum {
    let router = Router::new().route("/", get(hello));
    Ok(router.into())
}
```

## Databases

- **Shared DB** (free): `#[shuttle_shared_db::Postgres] pool: PgPool`
- **AWS RDS** (paid): `#[shuttle_aws_rds::Postgres] pool: PgPool`

## Secrets

Create `Secrets.toml` in project root, add to `.gitignore`:

```toml
MY_API_KEY = 'your-api-key-here'
```

Use in code:

```rust
#[shuttle_runtime::main]
async fn main(#[shuttle_runtime::Secrets] secrets: SecretStore) -> ShuttleAxum {
    let api_key = secrets.get("MY_API_KEY").unwrap();
    Ok(router.into())
}
```

## Static Assets

Configure in `Shuttle.toml`:

```toml
[build]
assets = [
    "assets/*",
    "frontend/dist/*",
    "static/*"
]

[deploy]
include = ["ignored-files/*"]  # Include files that are normally ignored by git
deny_dirty = true
```

## Development Workflow

1. `shuttle run` - local development
2. Use MCP server for AI-assisted development
3. Use MCP server for Searching the Docs

## Key Points

- Always use `#[shuttle_runtime::main]` as your entry point
- Configure static assets in `Shuttle.toml`
- Use secrets for sensitive configuration
