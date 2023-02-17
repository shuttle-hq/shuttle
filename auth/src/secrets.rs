use jsonwebtoken::{DecodingKey, EncodingKey};
use ring::signature::{Ed25519KeyPair, KeyPair};

pub trait KeyManager: Send + Sync {
    /// Get a private key for signing secrets
    fn private_key(&self) -> &EncodingKey;

    /// Get a public key to verify signed secrets
    fn public_key(&self) -> &DecodingKey;
}

pub struct EdDsaManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl EdDsaManager {
    pub fn new() -> Self {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new())
            .expect("to create a PKCS8 for edDSA");
        let encoding_key = EncodingKey::from_ed_der(doc.as_ref());
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).expect("to create a key pair");
        let decoding_key = DecodingKey::from_ed_der(pair.public_key().as_ref());

        Self {
            encoding_key,
            decoding_key,
        }
    }
}

impl KeyManager for EdDsaManager {
    fn private_key(&self) -> &EncodingKey {
        &self.encoding_key
    }

    fn public_key(&self) -> &DecodingKey {
        &self.decoding_key
    }
}
