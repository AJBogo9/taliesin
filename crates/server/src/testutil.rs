//! Test-only helpers shared across the server crate's `#[cfg(test)]` modules.

use serde_json::Value;

/// Parse a serialized websocket message back into JSON for assertions.
pub fn parse(s: String) -> Value {
    serde_json::from_str(&s).unwrap()
}
