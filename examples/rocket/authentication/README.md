# Issue and verify JWT for authentication
This example shows how to use [Rocket request guards](https://rocket.rs/v0.5-rc/guide/requests/#request-guards) for authentication with [JSON Web Tokens](https://jwt.io/) (JWT for short).
The idea is that all requests authenticate first at https://authentication-rocket-app.shuttleapp.rs/login to get a JWT.
Then the JWT is sent with all requests requiring authentication using the HTTP header `Authorization: Bearer <token>`.

This example uses the [`jsonwebtoken`](https://github.com/Keats/jsonwebtoken) which supports symmetric and asymmetric secret encoding, built-in validations, and most JWT algorithms.
However, this example only makes use of symmetric encoding and validation on the expiration claim.

## Structure
This example has two files to register routes and handle JWT claims.

### src/main.rs
Three Rocker routes are registered in this file:
1. `/public`: a route that can be called without needing any authentication.
1. `/login`: a route for posting a JSON object with a username and password to get a JWT.
1. `/private`: a route that can only be accessed with a valid JWT.

### src/claims.rs
The bulk of this example is in this file. Most of the code can be transferred to other frameworks except for the `FromRequest` implementation, which is Rocket specific.
This file contains a `Claims` object which can be expanded with more claims. A `Claims` can be created from a `Bearer <token>` string using `Claims::from_authorization()`.
And a `Claims` object can also be converted to a token using `to_token()`.

## Deploy
After logging into shuttle, use the following command to deploy this example:

```sh
$ cargo shuttle project new
$ cargo shuttle deploy
```

Now make a note of the `Host` for the deploy to use in the examples below. Or just use `authentication-rocket-app.shuttleapp.rs` as the host below.

### Seeing it in action
First, we should be able to access the public endpoint without any authentication using:

```sh
$ curl https://<host>/public
```

But trying to access the private endpoint will fail with a 403 forbidden:

```sh
$ curl https://<host>/private
```

So let's get a JWT from the login route first:


```sh
$ curl --request POST --data '{"username": "username", "password": "password"}' https://<host>/login
```

Accessing the private endpoint with the token will now succeed:

```sh
$ curl --header "Authorization: Bearer <token>" https://<host>/private
```

The token is set to expire in 5 minutus, so wait a while and try to access the private endpoint again. Once the token has expired, a user will need to get a new token from login.
Since tokens usually have a longer than 5 minutes expiration time, we can create a `/refresh` endpoint that takes an active token and returns a new token with a refreshed expiration time.
