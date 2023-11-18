use jsonwebtoken::EncodingKey;
use ring::signature::{Ed25519KeyPair, KeyPair};

pub trait KeyManager: Send + Sync {
    /// Get a private key for signing secrets
    fn private_key(&self) -> &EncodingKey;

    /// Get a public key to verify signed secrets
    fn public_key(&self) -> &[u8];
}

pub struct EdDsaManager {
    encoding_key: EncodingKey,
    public_key: Vec<u8>,
}

impl EdDsaManager {
    // We take the private key used for JWT signing as an auth argument. It is the base64 enconding of a PEM encoded
    // private key. The private key was generated previously using `OpenSSL 3.1.2 1 Aug 2023 (Library: OpenSSL 3.1.2 1
    // Aug 2023)`, by using `openssl genpkey -algorithm ED25519`. This generates a PEM encoded PKCS#8 v1 formatted
    // unencrypted key. We want to first parse the PEM encoded key and get the PKCS#8 v1 formatted key to parse it with
    // `ring::signature::Ed25519KeyPair`. This is needed because the public key is not contained within the openssl
    // generated private key, and should be computed/stored before serving it for JWT validation.
    pub fn new(jwt_signing_private_key: String) -> Self {
        // Decode the base64 encoding.
        let pk_bytes = base64::decode(jwt_signing_private_key)
            .expect("to decode base64 pem encoded private key");

        // Parse the pem file and the ed25519 private key contained.
        let pem_keypair = pem::parse(pk_bytes.clone()).expect("to parse pem encoded private key");
        let ed_keypair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(pem_keypair.contents())
            .expect("to get PKCS#8 v1 formatted private key from pem encoded key");

        Self {
            // Wrap the private key as a jwt encoding key.
            encoding_key: EncodingKey::from_ed_pem(pk_bytes.as_slice())
                .expect("to get an encoding key from pem encoded ed25519 private key"),
            public_key: ed_keypair.public_key().as_ref().to_vec(),
        }
    }
}

impl KeyManager for EdDsaManager {
    fn private_key(&self) -> &EncodingKey {
        &self.encoding_key
    }

    fn public_key(&self) -> &[u8] {
        &self.public_key
    }
}
