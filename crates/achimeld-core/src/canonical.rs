//! The legacy v1 encoding uses NFC, UTF-16 lengths and .NET enum type names.
use crate::{Decimal, DomainError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Text(String),
    Boolean(bool),
    Integer(i128),
    Decimal(Decimal),
    Enumeration {
        type_name: &'static str,
        name: &'static str,
    },
    Strings(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub entity: String,
    pub fields: BTreeMap<String, String>,
}

pub fn normalize(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect()
}
fn prefix(value: &str) -> String {
    format!("{}:{value}", value.encode_utf16().count())
}
pub fn hash(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

/// Match decimal.ToString("G29", InvariantCulture), including small exponents.
pub fn decimal_g29(value: Decimal) -> String {
    let value = value.normalize();
    if value.is_zero() {
        return "0".into();
    }
    let coefficient = value.mantissa().unsigned_abs().to_string();
    let exponent = coefficient.len() as i32 - value.scale() as i32 - 1;
    if exponent < -4 {
        let sign = if value.is_sign_negative() { "-" } else { "" };
        let tail = if coefficient.len() > 1 {
            format!(".{}", &coefficient[1..])
        } else {
            String::new()
        };
        format!("{sign}{}{tail}E-{:02}", &coefficient[..1], -exponent)
    } else {
        value.to_string()
    }
}

pub fn encode(value: &Value) -> String {
    match value {
        Value::Null => "null:".into(),
        Value::Text(text) => format!("string:{}", prefix(&normalize(text))),
        Value::Boolean(value) => format!("bool:{value}"),
        Value::Integer(value) => format!("integer:{value}"),
        Value::Decimal(value) => format!("decimal:{}", decimal_g29(*value)),
        Value::Enumeration { type_name, name } => format!("enum:{type_name}:{name}"),
        Value::Strings(values) => format!(
            "strings:[{}]",
            values
                .iter()
                .map(|value| prefix(&normalize(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
pub fn value_hash(value: &Value) -> String {
    hash(encode(value).as_bytes())
}

pub fn compute(mut fields: Vec<(String, Value)>) -> Result<Fingerprint, DomainError> {
    // Rust scalar/UTF-8 ordering differs from .NET ordinal ordering for astral text.
    fields.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let mut entity = String::from("achimeld-canonical-v1\n");
    let mut fingerprints = BTreeMap::new();
    for (name, value) in fields {
        if name.trim().is_empty() {
            return Err(DomainError("Canonical field name is empty"));
        }
        let line = format!("{}={}\n", prefix(&normalize(&name)), encode(&value));
        if fingerprints.insert(name, hash(line.as_bytes())).is_some() {
            return Err(DomainError("Duplicate canonical field"));
        }
        entity.push_str(&line);
    }
    Ok(Fingerprint {
        entity: hash(entity.as_bytes()),
        fields: fingerprints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    #[test]
    fn normalization_and_utf16_lengths_match_dotnet() {
        assert_eq!(
            encode(&Value::Text("e\u{301}\r\n😀".into())),
            "string:4:é\n😀"
        );
        assert_eq!(
            value_hash(&Value::Text("é\n".into())),
            value_hash(&Value::Text("e\u{301}\r\n".into()))
        );
    }
    #[test]
    fn decimals_keep_precision_and_dotnet_general_format() {
        for (input, expected) in [
            ("1.2300", "1.23"),
            ("0.00001", "1E-05"),
            ("-0.00000125", "-1.25E-06"),
            ("0.0001", "0.0001"),
            (
                "79228162514264337593543950335",
                "79228162514264337593543950335",
            ),
        ] {
            assert_eq!(
                decimal_g29(Decimal::from_str_exact(input).unwrap()),
                expected
            );
        }
        assert_eq!(decimal_g29(Decimal::from_str("-0.000").unwrap()), "0");
    }
    #[test]
    fn field_order_is_ordinal_and_duplicates_are_rejected() {
        let fields = vec![
            ("😀".into(), Value::Boolean(true)),
            ("\u{e000}".into(), Value::Null),
        ];
        let expected = "achimeld-canonical-v1\n2:😀=bool:true\n1:\u{e000}=null:\n";
        assert_eq!(compute(fields).unwrap().entity, hash(expected.as_bytes()));
        assert!(compute(vec![("x".into(), Value::Null), ("x".into(), Value::Null)]).is_err());
    }
}
