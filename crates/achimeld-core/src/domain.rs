use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError(pub &'static str);

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for DomainError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AppIdWire", into = "AppIdWire")]
pub struct SteamAppId(u32);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppIdWire {
    value: u32,
}

impl SteamAppId {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError("Steam App ID must be nonzero"));
        }
        Ok(Self(value))
    }
    pub fn value(self) -> u32 {
        self.0
    }
}
impl TryFrom<AppIdWire> for SteamAppId {
    type Error = DomainError;
    fn try_from(value: AppIdWire) -> Result<Self, Self::Error> {
        Self::new(value.value)
    }
}
impl From<SteamAppId> for AppIdWire {
    fn from(value: SteamAppId) -> Self {
        Self { value: value.0 }
    }
}
impl fmt::Display for SteamAppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ApiNameWire", into = "ApiNameWire")]
pub struct SteamApiName(String);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApiNameWire {
    value: String,
}

impl SteamApiName {
    pub fn new(value: &str) -> Result<Self, DomainError> {
        let value = value.trim();
        if !(1..=128).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
        {
            return Err(DomainError("Invalid Steam API name"));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<ApiNameWire> for SteamApiName {
    type Error = DomainError;
    fn try_from(value: ApiNameWire) -> Result<Self, Self::Error> {
        Self::new(&value.value)
    }
}
impl From<SteamApiName> for ApiNameWire {
    fn from(value: SteamApiName) -> Self {
        Self { value: value.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalizationId(String);
impl LocalizationId {
    pub fn new(value: &str) -> Result<Self, DomainError> {
        if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(DomainError(
                "Localization ID must contain six Base62 characters",
            ));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! legacy_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub fn legacy_name(self) -> &'static str { match self { $(Self::$variant => stringify!($variant)),+ } }
            pub fn legacy_type() -> &'static str { concat!("Achimeld.Core.", stringify!($name)) }
        }
        impl FromStr for $name {
            type Err = DomainError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value { $(stringify!($variant) => Ok(Self::$variant)),+, _ => Err(DomainError("Unknown domain enum value")) }
            }
        }
    }
}
legacy_enum!(StatType {
    Int,
    Float,
    AverageRate
});
legacy_enum!(SetBy {
    GameClient,
    GameServer,
    OfficialGameServer
});
legacy_enum!(ProjectEntityType { Achievement, Stat });
legacy_enum!(LocalizationField {
    AchievementDisplayName,
    AchievementDescription,
    StatDisplayName
});

pub const LANGUAGES: [&str; 7] = [
    "English",
    "Polish",
    "ChineseSimplified",
    "ChineseTraditional",
    "Japanese",
    "Korean",
    "French",
];

/// Interface choices are independent of the versioned project/Steam text schema.
pub const INTERFACE_LANGUAGES: [&str; 14] = [
    "English",
    "Polish",
    "ChineseSimplified",
    "ChineseTraditional",
    "Japanese",
    "Korean",
    "French",
    "German",
    "Spanish",
    "Italian",
    "Portuguese",
    "Dutch",
    "Russian",
    "Ukrainian",
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_wrappers_validate_legacy_values() {
        let app: SteamAppId = serde_json::from_str(r#"{"value":480}"#).unwrap();
        assert_eq!(serde_json::to_string(&app).unwrap(), r#"{"value":480}"#);
        assert!(serde_json::from_str::<SteamAppId>(r#"{"value":0}"#).is_err());
        assert!(serde_json::from_str::<SteamAppId>(r#"{"value":480,"extra":true}"#).is_err());
        assert_eq!(
            SteamApiName::new(" TEST:WIN ").unwrap().as_str(),
            "TEST:WIN"
        );
        for invalid in ["", "bad name", "é", "x' onclick=bad"] {
            assert!(SteamApiName::new(invalid).is_err());
        }
        assert!(SteamApiName::new(&"A".repeat(129)).is_err());
        assert!(LocalizationId::new("Ab0Z99").is_ok());
        assert!(LocalizationId::new("ABCDEF ").is_err());
    }
}
