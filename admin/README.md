# Admin

<!-- markdownlint-disable-next-line -->
*Small utility used by the shuttle admin for common tasks*

## How to test custom domain certificates locally

For local testing it is easiest to use the [Pebble](https://github.com/letsencrypt/pebble) server. So install it using
whatever method works for your system. It is included in the nix environment if you use it though.

To start the `Pebble` server you'll need some config, a root CA and a certificate signed with the CA. The easiest way
to get all these is to get them from the [pebble/test](https://github.com/letsencrypt/pebble/tree/main/test) folder.
At the same time, you'll need the [pebble/test/config/pebble-config.json](https://github.com/letsencrypt/pebble/tree/main/test/config/pebble-config.json)
to have its `httpPort` and `tlsPort` set to `7999`, the port of the bouncer (which handles the HTTP-01 challenge handling).

You should now be able to start `Pebble` locally. If you used the `pebble/test` folder, then your important variables are as follow:

- *Server url*: `https://localhost:14000/dir`
- *CA root certificate location*: `$PWD/test/certs/pebble.minica.pem`

Next you'll need `gateway` to use this CA when checking the TLS connection with localhost. This can be done by
setting the `SSL_CERT_FILE` environment variable. If you deploy the `gateway` through docker compose then you'll
need to export the `SSL_CERT_FILE` environmanet variable through the `docker-compose.yml` file, for the `gateway`
service.

**Note**: Building the containers locally will carry over to the images any "*.pem" files from the shuttle root
directory, given they are needed to enable the `SSL_CERT_FILE` on the gateway. You can have you Pebble CA root
certificate under shuttle root directory and this will be carried in the gateway container under `/usr/src/shuttle`.
Then the `SSL_CERT_FILE` can be set as `/usr/src/shuttle/{path_to_pebble.minica.pem}`.

``` shell
export SSL_CERT_FILE="$PWD/test/certs/pebble.minica.pem"
```

When `gateway` now runs, it will use this root certificate to check the certificate presented by `Pebble`. At the same
time, if `Pebble` runs on the `host` machine and your `gateway` runs in a container, you need to tell to the `gateway`
container that `https://localhost:14000/dir` points to the host machine pebble instance. You'll need to append to the
`gateway`s `/etc/hosts` a new entry for `localhost` to point also to the `host.docker.internal` IP. The `host.docker.internal`
IP can be found by running `ping host.docker.internal` in the `gateway` container.

Now you'll want this admin client to use the local `Pebble` server when making a new account. Therefore, use the
following command when you create new accounts:

``` shell
cargo run -p shuttle-admin -- --api-url http://localhost:8001 acme create-account --acme-server https://localhost:14000/dir --email <email>
```

Save the account JSON in a local file and use it to test creating a new certificate. Also, the FQDN you'll be using, to
request a certificate for, must resolve to your localhost, so you either add an entry for that in `/etc/hosts` or set up
a local DNS server (e.g. on MacOs: https://gist.github.com/ogrrd/5831371) to resolve specific domains to `127.0.0.1` (by adding
an `A` entry into the `dnsmasq.conf` file).

Requesting a new certificate for a custom-domain:

```shell
cargo run -p shuttle-admin -- --api-url http://localhost:8001 acme request --fqdn local.custom.domain.me --project <project-name> --credentials <pebble-account-credentials.json>
```

Renewing a new certificate for a custom-domain:
```shell
cargo run -p shuttle-admin -- --api-url http://localhost:8001 acme renew-custom-domain --fqdn local.custom.domain.me --project <project-name> --credentials <pebble-account-credentials.json>
```

## How to test gateway certificates locally

You will need the same setup done for `Pebble` as for the custom domain certificates. The difference is that `Pebble` will do
a DNS-01 challenge for requesting a gateway certificate. The gateway will log the actual challenge Pebble will check. It involves
adding a TXT entry in your local DNS (e.g. `txt-record=_acme-challenge.local.shuttle.test,ZqS9r9z6UY0anHV-DIjLGi0GKps0RG4HxoFJO3hmtYs`).
The gateway waits 1 minute before telling `Pebble` the challenge is ready, so you'll need to add the TXT record in the DNS within that time.
Also, to be able to create an order and a certificate, the `gateway` will need to load pre-existing account credentials from `acme.json` for
an account that lives in `Pebble` memory. Once everything is done correctly the `gateway` will generate its certificate under `ssl.pem`.

To renew the gateway certificate you'll run:
```shell
shuttle-admin acme renew-gateway --credentials <CREDENTIALS>
```


