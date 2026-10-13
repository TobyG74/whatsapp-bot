use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
};
use anyhow::{Result, anyhow, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

pub struct Crypto {
    cipher: Aes256Gcm,
    index_key: [u8; 32],
}

impl Crypto {
    pub fn new(key: &str) -> Result<Self> {
        let bytes = hex::decode(key)
            .map_err(|_| anyhow!("DATA_ENCRYPTION_KEY must be 64 hex characters"))?;
        let index_key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("DATA_ENCRYPTION_KEY must be 32 bytes"))?;
        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&index_key).expect("validated key length"),
            index_key,
        })
    }

    pub fn seal(&self, context: &str, data: &[u8]) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: data,
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| anyhow!("Encryption failed"))?;
        Ok(STANDARD.encode([nonce.as_slice(), &ciphertext].concat()))
    }

    pub fn open(&self, context: &str, data: &str) -> Result<Vec<u8>> {
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| anyhow!("Invalid encrypted record"))?;
        ensure!(bytes.len() >= 28, "Invalid encrypted record");
        self.cipher
            .decrypt(
                Nonce::from_slice(&bytes[..12]),
                Payload {
                    msg: &bytes[12..],
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| {
                anyhow!("Cannot decrypt record; check DATA_ENCRYPTION_KEY and database integrity")
            })
    }

    pub fn index(&self, kind: &str, value: &str) -> String {
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(&self.index_key).expect("valid HMAC key");
        mac.update(kind.as_bytes());
        mac.update(&[0]);
        mac.update(value.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

pub fn token_matches(expected: &str, provided: &str) -> bool {
    expected.len() == provided.len() && bool::from(expected.as_bytes().ct_eq(provided.as_bytes()))
}

pub fn random_secret() -> String {
    let first = Aes256Gcm::generate_nonce(&mut OsRng);
    let second = Aes256Gcm::generate_nonce(&mut OsRng);
    let third = Aes256Gcm::generate_nonce(&mut OsRng);
    hex::encode(&[first.as_slice(), second.as_slice(), third.as_slice()].concat()[..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_binds_records_and_detects_tampering() {
        let crypto = Crypto::new(&"11".repeat(32)).unwrap();
        let sealed = crypto.seal("contact:1", b"6281234567890").unwrap();
        assert_eq!(crypto.open("contact:1", &sealed).unwrap(), b"6281234567890");
        assert!(crypto.open("contact:2", &sealed).is_err());
        assert!(
            Crypto::new(&"22".repeat(32))
                .unwrap()
                .open("contact:1", &sealed)
                .is_err()
        );
        assert_ne!(sealed, crypto.seal("contact:1", b"6281234567890").unwrap());
        assert!(crypto.open("contact:1", "AA==").is_err());
        assert_ne!(crypto.index("contact", "1"), crypto.index("message", "1"));
    }
    #[test]
    fn rejects_bad_tokens() {
        assert!(token_matches("secret", "secret"));
        assert!(!token_matches("secret", "secrex"));
        assert!(!token_matches("secret", ""));
        assert!(Crypto::new("invalid").is_err());
        assert_eq!(random_secret().len(), 64);
    }
}
