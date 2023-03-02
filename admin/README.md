# Admin

<!-- markdownlint-disable-next-line -->
*Small utility used by the shuttle admin for common tasks*

## How to test custom domain certificates locally

For local testing it is easiest to use the [Pebble](https://github.com/letsencrypt/pebble) server. So install it using
whatever method works for your system. It is included in the nix environment if you use it though.

To start the `Pebble` server you'll need some config, a root CA and a certificate signed with the CA. The easiest way
to get all these is to get them from the [pebble/test](https://github.com/letsencrypt/pebble/tree/main/test) folder.

You should now be able to start `Pebble` locally. If you used the `pebble/test` folder, then your important
variables are as follow:

- *Server url*: `https://localhost:14000/dir`
- *CA location*: `$PWD/test/certs/pebble.minica.pem`

Next you'll need `gateway` to use this CA when checking the TLS connection with localhost. This can be done by
setting the `SSL_CERT_FILE` environment variable.

``` shell
export SSL_CERT_FILE="$PWD/test/certs/pebble.minica.pem"
```

When `gateway` now runs, it will use this root certificate to check the certificate presented by `Pebble`.

Now you'll want this admin client to use the local `Pebble` server when making new account. Therefore, use the
following command when you create new accounts

``` shell
cargo run -p shuttle-admin -- --api-url http://localhost:8001 acme create-account --acme-server https://localhost:14000/dir --email <email>
```

Safe the account JSON in a local file and use it to test creating new certificate. However, you'll the FQDN you're
using for testnig to resolve to your local machine. So create an `A` record for it on your DNS with the value
`127.0.0.1`. And Bob's your uncle 🎉
