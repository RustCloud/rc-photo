//! Small helpers for answers and query strings.

use crate::Response;

pub fn ok(value: serde_json::Value) -> Response {
    json(200, value)
}

pub fn error(status: u16, message: &str) -> Response {
    json(status, serde_json::json!({ "error": message }))
}

pub fn json(status: u16, value: serde_json::Value) -> Response {
    Response {
        status,
        headers: vec![("content-type".to_owned(), "application/json".to_owned())],
        body: value.to_string().into_bytes(),
    }
}

/// A file of the page itself.
pub fn asset(content_type: &str, body: &[u8]) -> Response {
    Response { status: 200, headers: vec![("content-type".to_owned(), content_type.to_owned())], body: body.to_vec() }
}

/// A file id as the server writes them (a UUID): nothing that could change
/// an AppData path it is put into.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// A parameter of a query string, decoded.
pub fn query_param(query: &str, name: &str) -> String {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| decode(value))
        .unwrap_or_default()
}

/// Decodes `%XX` and `+` in a query value.
fn decode(value: &str) -> String {
    let hex = |byte: u8| (byte as char).to_digit(16);
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(high), Some(low)) => {
                out.push((high * 16 + low) as u8);
                i += 3;
            }
            (b'+', _, _) => {
                out.push(b' ');
                i += 1;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(valid_id("0b9e2f3c-1d2e-4f5a-8b7c-112233445566"));
        assert!(!valid_id(""));
        assert!(!valid_id("../index/00"));
        assert!(!valid_id("a/b"));
        assert!(!valid_id(&"a".repeat(65)));
    }

    #[test]
    fn query() {
        assert_eq!(query_param("mode=h264&x=1", "mode"), "h264");
        assert_eq!(query_param("q=a+b%2Fc", "q"), "a b/c");
        assert_eq!(query_param("", "mode"), "");
    }
}
