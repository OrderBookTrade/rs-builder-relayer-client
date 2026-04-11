use super::BuilderConfig;
use base64::{engine::general_purpose, Engine};
use crate::error::{RelayerError, Result};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Build HMAC-SHA256 signature for a request.
///
/// The secret is base64-encoded. We decode it, then:
///   message = timestamp + method + path + body
///   signature = url_safe_base64(hmac_sha256(decoded_secret, message))
pub fn build_hmac_signature(
    secret: &str,
    timestamp: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<String> {
    let decoded_secret = general_purpose::STANDARD
        .decode(secret)
        .or_else(|_| general_purpose::URL_SAFE.decode(secret))
        .or_else(|_| general_purpose::URL_SAFE_NO_PAD.decode(secret))
        .map_err(|e| RelayerError::AuthError(format!("Failed to decode base64 secret: {e}")))?;

    let message = format!("{}{}{}{}", timestamp, method, path, body);

    let mut mac = HmacSha256::new_from_slice(&decoded_secret)
        .map_err(|e| RelayerError::AuthError(format!("HMAC key error: {e}")))?;
    mac.update(message.as_bytes());
    let result = mac.finalize().into_bytes();

    // Base64 encode, then make URL-safe (+ -> -, / -> _)
    let b64 = general_purpose::STANDARD.encode(result);
    let url_safe = b64.replace('+', "-").replace('/', "_");

    Ok(url_safe)
}

/// Build auth headers for a Builder API request.
pub fn build_headers(
    config: &BuilderConfig,
    method: &str,
    path: &str,
    body: &str,
) -> Result<HeaderMap> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RelayerError::AuthError("System time before UNIX EPOCH".to_string()))?
        .as_secs();
    let timestamp_str = timestamp.to_string();

    let signature = build_hmac_signature(&config.secret, &timestamp_str, method, path, body)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "POLY_BUILDER_API_KEY",
        HeaderValue::from_str(&config.key)
            .map_err(|_| RelayerError::AuthError("Invalid key header value".to_string()))?,
    );
    headers.insert(
        "POLY_BUILDER_TIMESTAMP",
        HeaderValue::from_str(&timestamp_str)
            .map_err(|_| RelayerError::AuthError("Invalid timestamp header value".to_string()))?,
    );
    headers.insert(
        "POLY_BUILDER_PASSPHRASE",
        HeaderValue::from_str(&config.passphrase)
            .map_err(|_| RelayerError::AuthError("Invalid passphrase header value".to_string()))?,
    );
    headers.insert(
        "POLY_BUILDER_SIGNATURE",
        HeaderValue::from_str(&signature)
            .map_err(|_| RelayerError::AuthError("Invalid signature header value".to_string()))?,
    );

    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_signature_matches_reference() {
        let secret = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        let timestamp = "1000000";
        let method = "test-sign";
        let path = "/orders";
        let body = r#"{"hash": "0x123"}"#;

        let sig = build_hmac_signature(secret, timestamp, method, path, body).unwrap();
        assert_eq!(sig, "ZwAdJKvoYRlEKDkNMwd5BuwNNtg93kNaR_oU2HrfVvc=");
    }
}
