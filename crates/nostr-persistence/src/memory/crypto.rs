use base64::Engine;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::crypto::{CryptoError, PayloadCodec, SecretBoxCodec};
use crate::events::NostrTag;
use crate::Error;

pub fn encode_payload<T: Serialize>(
    payload: &T,
    secretbox: Option<&SecretBoxCodec>,
) -> Result<(String, Option<NostrTag>), Error> {
    let json = serde_json::to_vec(payload)?;
    if let Some(codec) = secretbox {
        let encoded = codec.encode(&json)?;
        let content = base64::engine::general_purpose::STANDARD.encode(encoded);
        Ok((content, Some(SecretBoxCodec::enc_tag())))
    } else {
        let content = String::from_utf8(json)?;
        Ok((content, None))
    }
}

pub fn decode_payload<T: DeserializeOwned>(
    content: &str,
    enc_tag: Option<&str>,
    secretbox: Option<&SecretBoxCodec>,
) -> Result<T, Error> {
    if let Some(enc) = enc_tag {
        if enc != SecretBoxCodec::encoding_name() {
            return Err(Error::EncodingMismatch {
                expected: SecretBoxCodec::encoding_name().to_string(),
                actual: enc.to_string(),
            });
        }
        let codec = secretbox.ok_or_else(|| Error::Crypto(CryptoError::MissingKey))?;
        let decoded = base64::engine::general_purpose::STANDARD.decode(content)?;
        let plaintext = codec.decode(&decoded)?;
        Ok(serde_json::from_slice(&plaintext)?)
    } else {
        Ok(serde_json::from_str(content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct SamplePayload {
        value: String,
    }

    #[test]
    fn roundtrip_secretbox() {
        let key = "hex:0000000000000000000000000000000000000000000000000000000000000000";
        let codec = SecretBoxCodec::from_str(key).unwrap();
        let payload = SamplePayload {
            value: "hello".to_string(),
        };
        let (content, tag) = encode_payload(&payload, Some(&codec)).unwrap();
        let decoded: SamplePayload =
            decode_payload(&content, tag.as_ref().map(|t| t.values[0].as_str()), Some(&codec))
                .unwrap();
        assert_eq!(payload, decoded);
    }

    #[test]
    fn roundtrip_plaintext() {
        let payload = SamplePayload {
            value: "hello".to_string(),
        };
        let (content, tag) = encode_payload(&payload, None).unwrap();
        assert!(tag.is_none());
        let decoded: SamplePayload = decode_payload(&content, None, None).unwrap();
        assert_eq!(payload, decoded);
    }
}
