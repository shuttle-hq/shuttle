# shuttle-crontab

A service that calls URLs at specified cron intervals.

# Usage

Create a new `CrontabService` by providing a `shuttle_persist::PersistInstance`
and an `axum::Router`

```
// main.rs

use shuttle_crontab::{CrontabService, ShuttleCrontab};
use shuttle_persist::{Persist, PersistInstance};
#[shuttle_runtime::main]
async fn crontab(#[Persist] persist: PersistInstance) -> ShuttleCrontab {
    let router = Router::new().route("/trigger-me", get(|| async {
      "Triggered by the crontab service.".to_string()
    }));
    CrontabService::new(persist, router)
}
```

This will create an `axum::Service` with a cron runner mounted at `/crontab`.
The `/crontab/set` endpoint accepts a schedule and a URL as form data and
persists the cron job with `shuttle_persist` between runs.

```
curl -v http://localhost:8000/crontab/set\
  -H "Content-Type: application/x-www-form-urlencoded"\
  -d "schedule='*/2 * * * * *'&url='http://localhost:8000/trigger-me'"
```

This crate demonstrates implementation of a custom service with
[`shuttle_runtime::Service`](https://docs.shuttle.rs/examples/custom-service),
usage of [`shuttle_persist`](https://docs.shuttle.rs/resources/shuttle-persist),
and how to run an [`axum::Server`](https://github.com/tokio-rs/axum) and a
number of cron job processes in parallel. and how to set up an `axum::Server`
that communicates with the main `CronRunner` via
[tokio channels](https://tokio.rs/tokio/tutorial/channels).

# TODOs

- [x] Streamline error handling
- [ ] Let user pass their own route, combine the two
- [ ] Make name of `set-schedule` router configurable
- [ ] Use builder pattern for setting up and configuring the service
