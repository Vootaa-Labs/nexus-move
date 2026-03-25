#![forbid(unsafe_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

pub const CRATE_ROLE: &str = "type-surface";

pub type HashValue = [u8; 32];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionBoundary {
    pub upstream_runtime_frozen: bool,
    pub compiler_frontend_owned: bool,
}

impl ExtractionBoundary {
    pub const fn bootstrap() -> Self {
        Self {
            upstream_runtime_frozen: false,
            compiler_frontend_owned: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AccountAddress(pub [u8; 32]);

impl AccountAddress {
    pub const ZERO: Self = Self([0; 32]);

    pub fn from_hex_literal(input: &str) -> Result<Self, AddressParseError> {
        let stripped = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
            .unwrap_or(input);
        Self::from_hex(stripped)
    }

    pub fn from_hex(input: &str) -> Result<Self, AddressParseError> {
        if input.is_empty() {
            return Err(AddressParseError::Empty);
        }

        let normalized = if input.len() % 2 == 0 {
            input.to_owned()
        } else {
            let mut prefixed = String::with_capacity(input.len() + 1);
            prefixed.push('0');
            prefixed.push_str(input);
            prefixed
        };

        let decoded = decode_hex(&normalized)?;
        if decoded.len() > 32 {
            return Err(AddressParseError::TooLong {
                bytes: decoded.len(),
            });
        }

        let mut address = [0u8; 32];
        let start = 32 - decoded.len();
        address[start..].copy_from_slice(&decoded);
        Ok(Self(address))
    }

    pub const fn into_inner(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedAddressAssignment {
    pub name: String,
    pub address: AccountAddress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDigest {
    pub name: String,
    pub hash: HashValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressParseError {
    Empty,
    InvalidHex { index: usize, byte: u8 },
    TooLong { bytes: usize },
}

impl fmt::Display for AddressParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "address is empty"),
            Self::InvalidHex { index, byte } => {
                write!(
                    f,
                    "invalid hex byte '{}' at position {}",
                    *byte as char, index
                )
            }
            Self::TooLong { bytes } => {
                write!(f, "address exceeds 32 bytes: got {bytes} bytes")
            }
        }
    }
}

impl std::error::Error for AddressParseError {}

fn decode_hex(input: &str) -> Result<Vec<u8>, AddressParseError> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    let mut index = 0;

    while index < bytes.len() {
        let hi = decode_nibble(bytes[index], index)?;
        let lo = decode_nibble(bytes[index + 1], index + 1)?;
        decoded.push((hi << 4) | lo);
        index += 2;
    }

    Ok(decoded)
}

fn decode_nibble(byte: u8, index: usize) -> Result<u8, AddressParseError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AddressParseError::InvalidHex { index, byte }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_hex_literal() {
        let address = AccountAddress::from_hex_literal("0xCAFE").unwrap();
        assert_eq!(address.0[30], 0xCA);
        assert_eq!(address.0[31], 0xFE);
    }

    #[test]
    fn rejects_too_long_address() {
        let input = format!("0x{}", "11".repeat(33));
        let error = AccountAddress::from_hex_literal(&input).unwrap_err();
        assert_eq!(error, AddressParseError::TooLong { bytes: 33 });
    }

    #[test]
    fn display_is_full_width_hex() {
        let address = AccountAddress::from_hex_literal("0x1").unwrap();
        assert!(address.to_string().starts_with("0x00000000"));
        assert!(address.to_string().ends_with('1'));
    }
}
