# Url Shortener

A URL shortener built with shuttle, rocket and postgres/sqlx. you can use it from your terminal.

## How to use it

you can use this url shortener from terminal. just copy this command to your terminal and replace `<URL>` with url that you want to shorten

```bash
curl -X POST -d '<URL>' https://s.shuttleapp.rs
```

like this

```bash
curl -X POST -d 'https://docs.rs/shuttle-service/latest/shuttle_service/' https://s.shuttleapp.rs
```

and you will get shortened url back (something like this `https://s.shuttleapp.rs/RvpVU_`)

## How to deploy

To deploy this app, check out the repository locally

```bash
$ git clone https://github.com/getsynth/shuttle.git
```

navigate to `examples/url-shortener`

```bash
$ cd examples/url-shortener
```

install shuttle

```bash
$ cargo install cargo-shuttle
```

login to shuttle

```bash
$ cargo shuttle login
```

open up the `Shuttle.toml` file and change the project name to something 
unique - in shuttle, projects are globally unique. Then run

```bash
$ cargo shuttle deploy
```
