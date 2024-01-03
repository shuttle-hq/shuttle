# Auth service considerations

## JWT signing private key

Starting the service locally requires provisioning of a base64 encoded PEM encoded PKCS#8 v1 unencrypted private key.
The service was tested with keys generated as follows:

```bash
openssl genpkey -algorithm ED25519 -out auth_jwtsigning_private_key.pem
base64 < auth_jwtsigning_private_key.pem
```

Used `OpenSSL 3.1.2 1 Aug 2023 (Library: OpenSSL 3.1.2 1 Aug 2023)` and `FreeBSD base64`, on a `macOS Sonoma 14.1.1`.

## Running the binary on it's own

**The below commands are ran from the root of the repo**

- First, start the control db container:

```
docker compose -f docker-compose.rendered.yml up control-db
```

- Then insert an admin user into the database:

```
cargo run --bin shuttle-auth -- --db-connection-uri postgres://postgres:postgres@localhost:5434/postgres init-admin --name admin
```

- Then start the service, you can get a stripe-secret-key from the Stripe dashboard. **Always use the test Stripe API for this**. See instructions above for generating a jwt-signing-private-key.

```
cargo run --bin shuttle-auth -- \
    --db-connection-uri postgres://postgres:postgres@localhost:5434/postgres \
    start \
    --stripe-secret-key sk_test_<test key> \
    --jwt-signing-private-key <key> \
    --stripe-rds-price-id price_1OIS06FrN7EDaGOjaV0GXD7P
```

## Getting a JWT for manual testing

Some endpoints expect a JWT. To get a JWT for a specific user, you can run the following command:

```bash
curl localhost:8000/auth/key -H "x-shuttle-admin-secret: <admin user api key>" -H "Authorization: Bearer <api key of user to get jwt for>"
```

A token will be returned in the response, which you can pass in as a bearer token in requests to JWT guarded endpoints, e.g.

```bash
curl -X POST localhost:8000/users/subscription/items -H "Authorization: Bearer <jwt>" -H "Content-Type: application/json" -d '{"metadata_id":"test-database","item":"AwsRds","quantity":1}' -v
```
