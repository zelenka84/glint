//! Reading and writing Minecraft's NBT files as untyped trees.

use std::io::{Read, Write};
use std::path::Path;

use fastnbt::Value;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::{Error, Result, io};

fn nbt_error(path: &Path, message: impl ToString) -> Error {
    Error::Nbt {
        path: path.to_owned(),
        message: message.to_string(),
    }
}

/// Reads an NBT file, gzip-compressed (level.dat) or not (servers.dat).
pub fn read(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).map_err(|e| io(path, e))?;
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(&bytes[..])
            .read_to_end(&mut out)
            .map_err(|e| io(path, e))?;
        out
    } else {
        bytes
    };
    fastnbt::from_bytes(&raw).map_err(|e| nbt_error(path, e))
}

/// Writes `value` next to `path` and swaps it in, so a crash never leaves half a file.
pub fn write(path: &Path, value: &Value, gzip: bool) -> Result<()> {
    let raw = fastnbt::to_bytes(value).map_err(|e| nbt_error(path, e))?;
    let bytes = if gzip {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).map_err(|e| io(path, e))?;
        encoder.finish().map_err(|e| io(path, e))?
    } else {
        raw
    };
    let tmp = path.with_extension("glint-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| io(path, e))
}

/// Follows compound keys, e.g. `["Data", "Version", "Name"]`.
pub fn get<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().try_fold(value, |v, key| match v {
        Value::Compound(map) => map.get(*key),
        _ => None,
    })
}

pub fn get_mut<'a>(value: &'a mut Value, keys: &[&str]) -> Option<&'a mut Value> {
    keys.iter().try_fold(value, |v, key| match v {
        Value::Compound(map) => map.get_mut(*key),
        _ => None,
    })
}

pub fn as_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Byte(v) => Some(i64::from(*v)),
        Value::Short(v) => Some(i64::from(*v)),
        Value::Int(v) => Some(i64::from(*v)),
        Value::Long(v) => Some(*v),
        _ => None,
    }
}

pub fn as_str(value: Option<&Value>) -> Option<&str> {
    match value? {
        Value::String(s) => Some(s),
        _ => None,
    }
}
