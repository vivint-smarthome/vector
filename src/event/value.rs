use crate::event::{string_to_timestamp, timestamp_to_string};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use derive_is_enum_variant::is_enum_variant;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use toml::value::Value as TomlValue;

#[derive(PartialEq, Debug, Clone, is_enum_variant, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Value {
    // Order of variants is important here, because for an untagged enum deserialization is
    // attempted top to bottom, so when types can be confused for eachother we want the more
    // specific variant listed first.
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Map(BTreeMap<String, Value>),
    Array(Vec<Value>),
    #[serde(
        serialize_with = "serialize_as_string_timestamp",
        deserialize_with = "deserialize_as_string_timestamp"
    )]
    Timestamp(DateTime<Utc>),
    #[serde(
        serialize_with = "serialize_as_string_bytes",
        deserialize_with = "deserialize_as_string_bytes"
    )]
    Bytes(Bytes),
    #[serde(
        serialize_with = "serialize_as_none",
        deserialize_with = "deserialize_as_none"
    )]
    Null,
}

fn serialize_as_string_bytes<S: Serializer>(
    input: &Bytes,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&String::from_utf8_lossy(input))
}

fn serialize_as_string_timestamp<S: Serializer>(
    input: &DateTime<Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&timestamp_to_string(input))
}

fn deserialize_as_string_bytes<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Bytes, D::Error> {
    match String::deserialize(deserializer) {
        Ok(s) => Ok(Bytes::copy_from_slice(s.as_bytes())),
        Err(e) => Err(e),
    }
}

fn deserialize_as_string_timestamp<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<DateTime<Utc>, D::Error> {
    match String::deserialize(deserializer) {
        Ok(s) => string_to_timestamp(&s).map_err(|_| {
            <D::Error as de::Error>::invalid_type(de::Unexpected::Str(&s), &"timestamp")
        }),
        Err(e) => Err(e),
    }
}

fn serialize_as_none<S: Serializer>(serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_none()
}

fn deserialize_as_none<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    match Option::<()>::deserialize(deserializer) {
        Ok(Some(_)) => Err(<D::Error as de::Error>::unknown_variant(
            "???",
            &[
                "timestamp",
                "bytes",
                "integer",
                "float",
                "boolean",
                "map",
                "array",
                "null",
            ],
        )),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

impl From<Bytes> for Value {
    fn from(bytes: Bytes) -> Self {
        Value::Bytes(bytes)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(set: Vec<T>) -> Self {
        Value::from_iter(set.into_iter().map(|v| v.into()))
    }
}

impl From<String> for Value {
    fn from(string: String) -> Self {
        Value::Bytes(string.into())
    }
}

impl TryFrom<TomlValue> for Value {
    type Error = crate::Error;

    fn try_from(toml: TomlValue) -> crate::Result<Self> {
        Ok(match toml {
            TomlValue::String(s) => Self::from(s),
            TomlValue::Integer(i) => Self::from(i),
            TomlValue::Array(a) => Self::from(
                a.into_iter()
                    .map(Value::try_from)
                    .collect::<crate::Result<Vec<_>>>()?,
            ),
            TomlValue::Table(t) => Self::from(
                t.into_iter()
                    .map(|(k, v)| Value::try_from(v).map(|v| (k, v)))
                    .collect::<crate::Result<BTreeMap<_, _>>>()?,
            ),
            TomlValue::Datetime(dt) => Self::from(dt.to_string().parse::<DateTime<Utc>>()?),
            TomlValue::Boolean(b) => Self::from(b),
            TomlValue::Float(f) => Self::from(f),
        })
    }
}

// We only enable this in testing for convenience, since `"foo"` is a `&str`.
// In normal operation, it's better to let the caller decide where to clone and when, rather than
// hiding this from them.
#[cfg(test)]
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Bytes(Vec::from(s.as_bytes()).into())
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(timestamp: DateTime<Utc>) -> Self {
        Value::Timestamp(timestamp)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Float(f64::from(value))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<BTreeMap<String, Value>> for Value {
    fn from(value: BTreeMap<String, Value>) -> Self {
        Value::Map(value)
    }
}

impl FromIterator<Value> for Value {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        Value::Array(iter.into_iter().collect::<Vec<Value>>())
    }
}

impl FromIterator<(String, Value)> for Value {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        Value::Map(iter.into_iter().collect::<BTreeMap<String, Value>>())
    }
}

macro_rules! impl_valuekind_from_integer {
    ($t:ty) => {
        impl From<$t> for Value {
            fn from(value: $t) -> Self {
                Value::Integer(value as i64)
            }
        }
    };
}

impl_valuekind_from_integer!(i64);
impl_valuekind_from_integer!(i32);
impl_valuekind_from_integer!(i16);
impl_valuekind_from_integer!(i8);
impl_valuekind_from_integer!(isize);

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl From<serde_json::Value> for Value {
    fn from(json_value: serde_json::Value) -> Self {
        match json_value {
            serde_json::Value::Bool(b) => Value::Boolean(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    Value::Float(f)
                } else {
                    Value::Bytes(n.to_string().into())
                }
            }
            serde_json::Value::String(s) => Value::Bytes(Bytes::from(s)),
            serde_json::Value::Object(obj) => Value::Map(
                obj.into_iter()
                    .map(|(key, value)| (key, Value::from(value)))
                    .collect(),
            ),
            serde_json::Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Value::from).collect())
            }
            serde_json::Value::Null => Value::Null,
        }
    }
}

impl TryInto<serde_json::Value> for Value {
    type Error = crate::Error;

    fn try_into(self) -> std::result::Result<serde_json::Value, Self::Error> {
        match self {
            Value::Boolean(v) => Ok(serde_json::Value::from(v)),
            Value::Integer(v) => Ok(serde_json::Value::from(v)),
            Value::Float(v) => Ok(serde_json::Value::from(v)),
            Value::Bytes(v) => Ok(serde_json::Value::from(String::from_utf8(v.to_vec())?)),
            Value::Map(v) => Ok(serde_json::to_value(v)?),
            Value::Array(v) => Ok(serde_json::to_value(v)?),
            Value::Null => Ok(serde_json::Value::Null),
            Value::Timestamp(v) => Ok(serde_json::Value::from(timestamp_to_string(&v))),
        }
    }
}

impl Value {
    // TODO: return Cow
    pub fn to_string_lossy(&self) -> String {
        match self {
            Value::Bytes(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Value::Timestamp(timestamp) => timestamp_to_string(timestamp),
            Value::Integer(num) => format!("{}", num),
            Value::Float(num) => format!("{}", num),
            Value::Boolean(b) => format!("{}", b),
            Value::Map(map) => serde_json::to_string(map).expect("Cannot serialize map"),
            Value::Array(arr) => serde_json::to_string(arr).expect("Cannot serialize array"),
            Value::Null => "<null>".to_string(),
        }
    }

    pub fn as_bytes(&self) -> Bytes {
        match self {
            Value::Bytes(bytes) => bytes.clone(), // cloning a Bytes is cheap
            Value::Timestamp(timestamp) => Bytes::from(timestamp_to_string(timestamp)),
            Value::Integer(num) => Bytes::from(format!("{}", num)),
            Value::Float(num) => Bytes::from(format!("{}", num)),
            Value::Boolean(b) => Bytes::from(format!("{}", b)),
            Value::Map(map) => Bytes::from(serde_json::to_vec(map).expect("Cannot serialize map")),
            Value::Array(arr) => {
                Bytes::from(serde_json::to_vec(arr).expect("Cannot serialize array"))
            }
            Value::Null => Bytes::from("<null>"),
        }
    }

    pub fn into_bytes(self) -> Bytes {
        self.as_bytes()
    }

    pub fn as_timestamp(&self) -> Option<&DateTime<Utc>> {
        match &self {
            Value::Timestamp(ts) => Some(ts),
            _ => None,
        }
    }

    pub fn kind(&self) -> &str {
        match self {
            Value::Bytes(_) => "string",
            Value::Timestamp(_) => "timestamp",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
            Value::Map(_) => "map",
            Value::Array(_) => "array",
            Value::Null => "null",
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{fs, io::Read, path::Path};

    fn parse_artifact(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
        let mut test_file = match fs::File::open(path) {
            Ok(file) => file,
            Err(e) => return Err(e),
        };

        let mut buf = Vec::new();
        test_file.read_to_end(&mut buf)?;

        Ok(buf)
    }

    // This test iterates over the `tests/data/fixtures/value` folder and:
    //   * Ensures the parsed folder name matches the parsed type of the `Value`.
    //   * Ensures the `serde_json::Value` to `vector::Value` conversions are harmless. (Think UTF-8 errors)
    //
    // Basically: This test makes sure we aren't mutilating any content users might be sending.
    #[test]
    fn json_value_to_vector_value_to_json_value() {
        crate::test_util::trace_init();
        const FIXTURE_ROOT: &str = "tests/data/fixtures/value";

        tracing::trace!(?FIXTURE_ROOT, "Opening");
        std::fs::read_dir(FIXTURE_ROOT).unwrap().for_each(|type_dir| match type_dir {
            Ok(type_name) => {
                let path = type_name.path();
                tracing::trace!(?path, "Opening");
                std::fs::read_dir(path).unwrap().for_each(|fixture_file| match fixture_file {
                    Ok(fixture_file) => {
                        let path = fixture_file.path();
                        let buf = parse_artifact(&path).unwrap();

                        let serde_value: serde_json::Value = serde_json::from_slice(&*buf).unwrap();
                        let vector_value = Value::from(serde_value.clone());

                        // Validate type
                        let expected_type = type_name.path().file_name().unwrap().to_string_lossy().to_string();
                        assert!(match &*expected_type {
                            "boolean" => vector_value.is_boolean(),
                            "integer" => vector_value.is_integer(),
                            "bytes" => vector_value.is_bytes(),
                            "array" => vector_value.is_array(),
                            "map" => vector_value.is_map(),
                            "null" => vector_value.is_null(),
                            _ => unreachable!("You need to add a new type handler here."),
                        }, "Typecheck failure. Wanted {}, got {:?}.", expected_type, vector_value);

                        let serde_value_again: serde_json::Value = vector_value.clone().try_into().unwrap();

                        tracing::trace!(?path, ?serde_value, ?vector_value, ?serde_value_again, "Asserting equal.");
                        assert_eq!(
                            serde_value,
                            serde_value_again
                        );
                    },
                    _ => panic!("This test should never read Err'ing test fixtures."),
                });
            },
            _ => panic!("This test should never read Err'ing type folders."),
        })
    }

    #[test]
    fn serialize_and_deserialize_custom_functions() {
        let timestamp = Value::Timestamp(Utc::now());
        assert_eq!(
            serde_json::from_str::<Value>(&serde_json::to_string(&timestamp).unwrap()).unwrap(),
            timestamp
        );
        let bytes = Value::Bytes(Bytes::from("hello world!"));
        assert_eq!(
            serde_json::from_str::<Value>(&serde_json::to_string(&bytes).unwrap()).unwrap(),
            bytes
        );
        assert_eq!(
            serde_json::from_str::<Value>(&serde_json::to_string(&Value::Null).unwrap()).unwrap(),
            Value::Null
        );
    }
}
