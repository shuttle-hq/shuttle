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
    pub fn new() -> Self {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
            .expect("to create a PKCS8 for edDSA");
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).expect("to create a key pair");
        let public_key = pair.public_key();

        Self {
            encoding_key,
            public_key: public_key.as_ref().to_vec(),
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
