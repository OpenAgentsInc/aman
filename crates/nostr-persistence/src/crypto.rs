use std::env;

use base64::Engine;
use rand_core::{OsRng, RngCore};
use thiserror::Error;
use xsalsa20poly1305::aead::{Aead, KeyInit};
use xsalsa20poly1305::{Key, Nonce, XSalsa20Poly1305};

use crate::events::enc_tag;

const SECRETBOX_TAG: &str = "secretbox-v1";
const SECRETBOX_KEY_LEN: usize = 32;
const SECRETBOX_NONCE_LEN: usize = 24;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("missing secretbox key")]
    MissingKey,
    #[error("invalid secretbox key length: {0}")]
    InvalidKeyLength(usize),
    #[error("invalid ciphertext length: {0}")]
    InvalidCiphertextLength(usize),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("hex error: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("crypto failure")]
    Aead,
}

pub trait PayloadCodec: Send + Sync {
    fn encode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError>;
    fn decode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError>;
    fn encoding_tag(&self) -> Option<&'static str>;
}

#[derive(Debug, Default)]
pub struct NoopCodec;

impl PayloadCodec for NoopCodec {
    fn encode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError> {
        Ok(input.to_vec())
    }

    fn decode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError> {
        Ok(input.to_vec())
    }

    fn encoding_tag(&self) -> Option<&'static str> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct SecretBoxCodec {
    key: [u8; SECRETBOX_KEY_LEN],
}

impl SecretBoxCodec {
    pub fn from_env(var: &str) -> Result<Self, CryptoError> {
        let value = env::var(var).map_err(|_| CryptoError::MissingKey)?;
        Self::from_str(&value)
    }

    pub fn from_str(value: &str) -> Result<Self, CryptoError> {
        let key_bytes = decode_key(value)?;
        Ok(Self { key: key_bytes })
    }

    pub fn enc_tag() -> crate::events::NostrTag {
        enc_tag(SECRETBOX_TAG)
    }

    pub fn encoding_name() -> &'static str {
        SECRETBOX_TAG
    }
}

impl PayloadCodec for SecretBoxCodec {
    fn encode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cipher = XSalsa20Poly1305::new(Key::from_slice(&self.key));
        let mut nonce_bytes = [0u8; SECRETBOX_NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, input).map_err(|_| CryptoError::Aead)?;
        let mut out = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decode(&self, input: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if input.len() < SECRETBOX_NONCE_LEN {
            return Err(CryptoError::InvalidCiphertextLength(input.len()));
        }
        let (nonce_bytes, ciphertext) = input.split_at(SECRETBOX_NONCE_LEN);
        let cipher = XSalsa20Poly1305::new(Key::from_slice(&self.key));
        let nonce = Nonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::Aead)
    }

    fn encoding_tag(&self) -> Option<&'static str> {
        Some(SECRETBOX_TAG)
    }
}

fn decode_key(value: &str) -> Result<[u8; SECRETBOX_KEY_LEN], CryptoError> {
    let trimmed = value.trim();
    let bytes = if let Some(hex_value) = trimmed.strip_prefix("hex:") {
        hex::decode(hex_value)?
    } else if is_probably_hex(trimmed) {
        hex::decode(trimmed)?
    } else {
        base64::engine::general_purpose::STANDARD.decode(trimmed)?
    };

    if bytes.len() != SECRETBOX_KEY_LEN {
        return Err(CryptoError::InvalidKeyLength(bytes.len()));
    }

    let mut key = [0u8; SECRETBOX_KEY_LEN];
    key.copy_from_slice(&bytes);
    Ok(key)
}

fn is_probably_hex(value: &str) -> bool {
    value.len() == SECRETBOX_KEY_LEN * 2 && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn codec_tag(codec: &dyn PayloadCodec) -> Option<crate::events::NostrTag> {
    codec.encoding_tag().map(enc_tag)
}
