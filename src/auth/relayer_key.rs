use reqwest::header::{HeaderMap, HeaderValue};
use crate::error::{RelayerError, Result};

pub fn build_headers(api_key: &str, address: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    headers.insert(
        "RELAYER_API_KEY",
        HeaderValue::from_str(api_key)
            .map_err(|_| RelayerError::AuthError("Invalid API key header value".to_string()))?,
    );
    headers.insert(
        "RELAYER_API_KEY_ADDRESS",
        HeaderValue::from_str(address)
            .map_err(|_| RelayerError::AuthError("Invalid address header value".to_string()))?,
    );

    Ok(headers)
}
