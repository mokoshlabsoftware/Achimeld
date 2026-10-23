use crate::SecurityError;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{Read, Write};

pub const MAXIMUM_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
pub const MAXIMUM_JSON_DEPTH: usize = 64;

pub fn check_json_depth(bytes: &[u8]) -> Result<(), SecurityError> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > MAXIMUM_JSON_DEPTH {
                        return Err(SecurityError("IPC JSON nesting limit exceeded"));
                    }
                }
                b'}' | b']' => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or(SecurityError("Invalid IPC JSON"))?;
                }
                _ => {}
            }
        }
    }
    if quoted || depth != 0 {
        return Err(SecurityError("Invalid IPC JSON"));
    }
    Ok(())
}

pub fn write_frame<W: Write, T: Serialize>(
    writer: &mut W,
    value: &T,
    maximum: usize,
) -> Result<(), SecurityError> {
    let bytes = serde_json::to_vec(value).map_err(|_| SecurityError("Invalid IPC request"))?;
    if bytes.is_empty() || bytes.len() > maximum.min(MAXIMUM_MESSAGE_BYTES) {
        return Err(SecurityError("IPC message exceeds limit"));
    }
    check_json_depth(&bytes)?;
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .and_then(|_| writer.flush())
        .map_err(|_| SecurityError("IPC write failed"))
}

pub fn read_frame<R: Read, T: DeserializeOwned>(
    reader: &mut R,
    maximum: usize,
) -> Result<T, SecurityError> {
    let mut header = [0; 4];
    reader
        .read_exact(&mut header)
        .map_err(|_| SecurityError("IPC header unavailable"))?;
    let length = u32::from_le_bytes(header) as usize;
    if length == 0 || length > maximum.min(MAXIMUM_MESSAGE_BYTES) {
        return Err(SecurityError("Invalid IPC message length"));
    }
    let mut bytes = vec![0; length];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| SecurityError("Incomplete IPC message"))?;
    check_json_depth(&bytes)?;
    serde_json::from_slice(&bytes).map_err(|_| SecurityError("Invalid IPC response"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    #[test]
    fn bounded_mutations_never_panic_or_accept_invalid_frames() {
        let seed = br#"{"text":"escaped \"quote","nested":[null,true,123]}"#;
        assert!(serde_json::from_slice::<serde_json::Value>(seed).is_ok());
        for position in 0..seed.len() {
            for replacement in [0, b'"', b'\\', b'[', b']', b'{', b'}', 0xff] {
                let mut payload = seed.to_vec();
                payload[position] = replacement;
                let _ = check_json_depth(&payload);
                let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
                frame.extend_from_slice(&payload);
                if let Ok(value) =
                    read_frame::<_, serde_json::Value>(&mut Cursor::new(&frame), 4096)
                {
                    assert_eq!(
                        value,
                        serde_json::from_slice::<serde_json::Value>(&payload).unwrap()
                    );
                }
                frame.pop();
                assert!(
                    read_frame::<_, serde_json::Value>(&mut Cursor::new(&frame), 4096).is_err()
                );
            }
        }
        for depth in [63, 64, 65, 256] {
            let bytes = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            assert_eq!(
                check_json_depth(bytes.as_bytes()).is_ok(),
                depth <= MAXIMUM_JSON_DEPTH
            );
        }
    }
    #[test]
    fn legacy_frame_is_length_prefixed_utf8_and_handles_partial_input() {
        let value = json!({"operation":"readConfiguration","arguments":{"value":480}});
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &value, MAXIMUM_MESSAGE_BYTES).unwrap();
        assert_eq!(
            u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize,
            bytes.len() - 4
        );
        assert_eq!(
            read_frame::<_, serde_json::Value>(&mut Cursor::new(&bytes), MAXIMUM_MESSAGE_BYTES)
                .unwrap(),
            value
        );
        bytes.pop();
        assert!(
            read_frame::<_, serde_json::Value>(&mut Cursor::new(bytes), MAXIMUM_MESSAGE_BYTES)
                .is_err()
        );
    }
    #[test]
    fn rejects_oversized_empty_and_deep_messages_before_deserialization() {
        for size in [0u32, MAXIMUM_MESSAGE_BYTES as u32 + 1, u32::MAX] {
            assert!(
                read_frame::<_, serde_json::Value>(
                    &mut Cursor::new(size.to_le_bytes()),
                    MAXIMUM_MESSAGE_BYTES
                )
                .is_err()
            );
        }
        assert!(
            check_json_depth(format!("{}0{}", "[".repeat(65), "]".repeat(65)).as_bytes()).is_err()
        );
        assert!(check_json_depth(br#"{"text":"[\\\"{}]"}"#).is_ok());
    }
}
