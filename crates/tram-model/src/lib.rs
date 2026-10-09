// SPDX-License-Identifier: Apache-2.0
//! Canonical, owned TRAM values and lineage. Runtime adapters must not coerce
//! telecom counters, bytes, null, absent fields, or branch-local mutations.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    EmptyIdentity,
    InvalidNumber,
    NumericOverflow,
    NonFiniteFloat,
    InvalidTimestamp,
    UnsupportedConversion {
        from: &'static str,
        to: &'static str,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity => write!(f, "identity must not be empty"),
            Self::InvalidNumber => write!(f, "invalid exact numeric representation"),
            Self::NumericOverflow => write!(f, "numeric value outside target range"),
            Self::NonFiniteFloat => write!(f, "nonfinite float is forbidden"),
            Self::InvalidTimestamp => write!(f, "invalid timestamp components"),
            Self::UnsupportedConversion { from, to } => {
                write!(f, "unsupported conversion from {from} to {to}")
            }
        }
    }
}
impl Error for ModelError {}

macro_rules! identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ModelError> {
                let value = value.into();
                if value.is_empty() || value.len() > 512 || value.trim() != value {
                    return Err(ModelError::EmptyIdentity);
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
identity!(RunId);
identity!(RecordId);
identity!(SourceUnitId);
identity!(PipelineRevision);
identity!(BranchId);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Timestamp {
    /// Unix epoch seconds; leap-second policy belongs to the codec.
    pub unix_seconds: i64,
    pub nanos: u32,
    /// Offset in seconds from UTC, preserved rather than silently discarded.
    pub offset_seconds: i32,
}
impl Timestamp {
    pub fn new(unix_seconds: i64, nanos: u32, offset_seconds: i32) -> Result<Self, ModelError> {
        if nanos >= 1_000_000_000 || !(-86_400..=86_400).contains(&offset_seconds) {
            return Err(ModelError::InvalidTimestamp);
        }
        Ok(Self {
            unix_seconds,
            nanos,
            offset_seconds,
        })
    }
}

fn canonical_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !(digits.len() > 1 && digits.starts_with('0'))
        && text != "-0"
}
fn canonical_decimal(text: &str) -> bool {
    if let Some((whole, fraction)) = text.split_once('.') {
        canonical_integer(whole)
            && !fraction.is_empty()
            && fraction.bytes().all(|b| b.is_ascii_digit())
    } else {
        canonical_integer(text)
    }
}

/// Owned numbers prevent precision loss through f64 intermediate values.
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Null,
    Boolean(bool),
    Signed(i64),
    Unsigned(u64),
    BigInteger(String),
    Decimal(String),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Timestamp(Timestamp),
    Array(Vec<Datum>),
    Object(BTreeMap<String, Datum>),
}
impl Datum {
    pub fn bigint(text: impl Into<String>) -> Result<Self, ModelError> {
        let text = text.into();
        if !canonical_integer(&text) {
            return Err(ModelError::InvalidNumber);
        }
        Ok(Self::BigInteger(text))
    }
    pub fn decimal(text: impl Into<String>) -> Result<Self, ModelError> {
        let text = text.into();
        if !canonical_decimal(&text) {
            return Err(ModelError::InvalidNumber);
        }
        Ok(Self::Decimal(text))
    }
    pub fn float(value: f64) -> Result<Self, ModelError> {
        if !value.is_finite() {
            return Err(ModelError::NonFiniteFloat);
        }
        Ok(Self::Float(value))
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean(_) => "boolean",
            Self::Signed(_) => "signed",
            Self::Unsigned(_) => "unsigned",
            Self::BigInteger(_) => "big-integer",
            Self::Decimal(_) => "decimal",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::Timestamp(_) => "timestamp",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
    pub fn to_i64_exact(&self) -> Result<i64, ModelError> {
        match self {
            Self::Signed(x) => Ok(*x),
            Self::Unsigned(x) => i64::try_from(*x).map_err(|_| ModelError::NumericOverflow),
            Self::BigInteger(x) => x.parse().map_err(|_| ModelError::NumericOverflow),
            _ => Err(ModelError::UnsupportedConversion {
                from: self.kind(),
                to: "i64",
            }),
        }
    }
    pub fn to_u64_exact(&self) -> Result<u64, ModelError> {
        match self {
            Self::Unsigned(x) => Ok(*x),
            Self::Signed(x) => u64::try_from(*x).map_err(|_| ModelError::NumericOverflow),
            Self::BigInteger(x) => x.parse().map_err(|_| ModelError::NumericOverflow),
            _ => Err(ModelError::UnsupportedConversion {
                from: self.kind(),
                to: "u64",
            }),
        }
    }
    /// No implicit float, byte-to-string or timestamp-to-string coercion.
    pub fn as_string(&self) -> Result<&str, ModelError> {
        match self {
            Self::String(x) => Ok(x),
            _ => Err(ModelError::UnsupportedConversion {
                from: self.kind(),
                to: "string",
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldPresence<'a> {
    Missing,
    Present(&'a Datum),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provenance {
    pub run_id: RunId,
    pub source_unit_id: SourceUnitId,
    pub record_id: RecordId,
    pub source_plugin: String,
    pub ingested_at: Option<Timestamp>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lineage {
    pub original_record: RecordId,
    pub parent_record: Option<RecordId>,
    pub branch: Option<BranchId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub opaque_cursor: Vec<u8>,
    pub ordinal: Option<u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RecordEnvelope {
    pub data: BTreeMap<String, Datum>,
    pub metadata: BTreeMap<String, Datum>,
    pub provenance: Provenance,
    pub lineage: Lineage,
    pub source_position: Option<SourcePosition>,
    pub raw_source_payload: Option<Vec<u8>>,
}
impl RecordEnvelope {
    pub fn new(provenance: Provenance) -> Self {
        let original_record = provenance.record_id.clone();
        Self {
            data: BTreeMap::new(),
            metadata: BTreeMap::new(),
            provenance,
            lineage: Lineage {
                original_record,
                parent_record: None,
                branch: None,
            },
            source_position: None,
            raw_source_payload: None,
        }
    }
    pub fn field(&self, name: &str) -> FieldPresence<'_> {
        match self.data.get(name) {
            Some(value) => FieldPresence::Present(value),
            None => FieldPresence::Missing,
        }
    }
    /// Deep owned clone: one sink branch can never mutate another's fields.
    pub fn fork_for_branch(&self, branch: BranchId) -> Self {
        let mut record = self.clone();
        record.lineage.parent_record = Some(self.provenance.record_id.clone());
        record.lineage.branch = Some(branch);
        record
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> RecordEnvelope {
        RecordEnvelope::new(Provenance {
            run_id: RunId::new("run").expect("valid run"),
            source_unit_id: SourceUnitId::new("source:1").expect("valid unit"),
            record_id: RecordId::new("record:1").expect("valid record"),
            source_plugin: "local".into(),
            ingested_at: None,
        })
    }

    #[test]
    fn missing_is_not_null() {
        let mut r = record();
        assert_eq!(r.field("cell"), FieldPresence::Missing);
        r.data.insert("cell".into(), Datum::Null);
        assert_eq!(r.field("cell"), FieldPresence::Present(&Datum::Null));
    }

    #[test]
    fn all_u64_bits_survive_without_float_conversion() {
        let value = Datum::Unsigned(u64::MAX);
        assert_eq!(value.to_u64_exact(), Ok(u64::MAX));
        assert_eq!(value.to_i64_exact(), Err(ModelError::NumericOverflow));
        let big = Datum::bigint("18446744073709551616").expect("valid large integer");
        assert_eq!(big.to_u64_exact(), Err(ModelError::NumericOverflow));
        assert_eq!(
            Datum::Signed(-1).to_u64_exact(),
            Err(ModelError::NumericOverflow)
        );
    }

    #[test]
    fn exact_representation_and_unsupported_coercion() {
        assert_eq!(Datum::bigint("01"), Err(ModelError::InvalidNumber));
        assert_eq!(Datum::bigint("-0"), Err(ModelError::InvalidNumber));
        assert_eq!(Datum::decimal("1.20"), Ok(Datum::Decimal("1.20".into())));
        assert_eq!(Datum::decimal("1..20"), Err(ModelError::InvalidNumber));
        assert_eq!(Datum::float(f64::NAN), Err(ModelError::NonFiniteFloat));
        assert!(matches!(
            Datum::Bytes(vec![0xff]).as_string(),
            Err(ModelError::UnsupportedConversion { .. })
        ));
        assert!(matches!(
            Datum::Boolean(true).to_i64_exact(),
            Err(ModelError::UnsupportedConversion { .. })
        ));
    }

    #[test]
    fn deep_nested_values_and_branch_isolation() {
        let mut original = record();
        original.data.insert(
            "nested".into(),
            Datum::Object(BTreeMap::from([(
                "array".into(),
                Datum::Array(vec![Datum::Bytes(vec![1, 2, 3])]),
            )])),
        );
        original.raw_source_payload = Some(vec![1, 2, 3]);
        let mut a = original.fork_for_branch(BranchId::new("A").expect("valid branch"));
        let b = original.fork_for_branch(BranchId::new("B").expect("valid branch"));
        if let Some(Datum::Object(obj)) = a.data.get_mut("nested") {
            obj.insert("array".into(), Datum::Array(vec![Datum::Null]));
        }
        a.raw_source_payload.as_mut().expect("raw payload")[0] = 9;
        assert_eq!(b.data.get("nested"), original.data.get("nested"));
        assert_eq!(b.raw_source_payload, Some(vec![1, 2, 3]));
        assert_ne!(a.data, b.data);
    }

    #[test]
    fn validated_identity_and_timestamp() {
        assert!(RunId::new("").is_err());
        assert!(RunId::new(" id ").is_err());
        assert!(Timestamp::new(0, 1_000_000_000, 0).is_err());
        assert!(Timestamp::new(0, 999_999_999, 0).is_ok());
    }
}
