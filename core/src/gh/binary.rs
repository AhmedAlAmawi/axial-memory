//! Binary GH_IO archives (`.gh`, `.ghuser`, `.ghcluster`): a raw-deflate stream
//! wrapping a tree of chunks. Every chunk is `name, index, item count, chunk
//! count, items…, chunks…`; strings are .NET `BinaryWriter` strings (7-bit
//! length + UTF-8). Verified byte-for-byte against McNeel's own sample files.

use super::archive::{guid_from_dotnet_bytes, types, Chunk, Item, Value};
use flate2::read::{DeflateDecoder, ZlibDecoder};
use std::fmt;
use std::io::Read;

/// Largest decompressed archive we will hold in memory.
pub const MAX_INFLATED: usize = 512 * 1024 * 1024;
const MAX_DEPTH: usize = 96;

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryError {
    NotCompressed,
    TooLarge,
    Truncated { at: usize },
    BadString { at: usize },
    BadCount { at: usize },
    UnknownType { at: usize, code: i32 },
    TooDeep,
}

impl fmt::Display for BinaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryError::NotCompressed => write!(f, "it isn't a Grasshopper file inside, whatever its name says"),
            BinaryError::TooLarge => write!(f, "it's too large to read inside"),
            BinaryError::Truncated { .. } => {
                write!(f, "the file stops part-way through, so it's probably incomplete or damaged")
            }
            BinaryError::BadString { .. } | BinaryError::BadCount { .. } => {
                write!(f, "part of the file is unreadable, so it's probably damaged")
            }
            BinaryError::UnknownType { code, .. } => {
                write!(f, "it uses something this version can't read yet (type {code})")
            }
            BinaryError::TooDeep => write!(f, "it's nested too deeply to read"),
        }
    }
}

pub fn inflate(compressed: &[u8], limit: usize) -> Result<Vec<u8>, BinaryError> {
    let mut out = Vec::with_capacity(compressed.len().saturating_mul(4).min(limit));
    let raw = DeflateDecoder::new(compressed).take(limit as u64 + 1).read_to_end(&mut out);
    if raw.is_err() || out.is_empty() {
        out.clear();
        let zlib = ZlibDecoder::new(compressed).take(limit as u64 + 1).read_to_end(&mut out);
        if zlib.is_err() || out.is_empty() {
            return Err(BinaryError::NotCompressed);
        }
    }
    if out.len() > limit {
        return Err(BinaryError::TooLarge);
    }
    Ok(out)
}

/// Parses a compressed archive (the contents of a `.gh` file).
pub fn parse_compressed(bytes: &[u8]) -> Result<Chunk, BinaryError> {
    let raw = inflate(bytes, MAX_INFLATED)?;
    parse_raw(&raw).map(|(chunk, _)| chunk)
}

/// Parses an already-decompressed chunk tree. Returns the root and how many bytes it used.
pub fn parse_raw(raw: &[u8]) -> Result<(Chunk, usize), BinaryError> {
    let mut r = Reader { b: raw, p: 0 };
    let root = r.chunk(0)?;
    Ok((root, r.p))
}

/// A byte array inside an archive is sometimes a whole archive of its own
/// (cluster documents, the component inside a user object). Accept it only if
/// it parses cleanly from end to end, so random bytes never pass.
pub fn try_nested(bytes: &[u8]) -> Option<Chunk> {
    if bytes.len() < 12 {
        return None;
    }
    let strict = |raw: &[u8]| -> Option<Chunk> {
        let (chunk, used) = parse_raw(raw).ok()?;
        let plausible = !chunk.name.is_empty()
            && chunk.name.len() <= 64
            && chunk.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        (plausible && used == raw.len()).then_some(chunk)
    };
    if let Ok(raw) = inflate(bytes, 128 * 1024 * 1024) {
        if let Some(c) = strict(&raw) {
            return Some(c);
        }
    }
    strict(bytes)
}

/// True if the first bytes of a file look like a compressed GH archive, for
/// files whose names don't say what they are (backups, missing extensions).
pub fn sniff(head: &[u8]) -> bool {
    let mut d = flate2::Decompress::new(false);
    let mut out = [0u8; 64];
    if d.decompress(head, &mut out, flate2::FlushDecompress::None).is_err() {
        return false;
    }
    let n = d.total_out() as usize;
    let out = &out[..n];
    let Some(&len) = out.first() else { return false };
    let len = len as usize;
    if !(3..=32).contains(&len) || out.len() < 1 + len + 4 {
        return false;
    }
    let name = &out[1..1 + len];
    let index = i32::from_le_bytes([out[1 + len], out[2 + len], out[3 + len], out[4 + len]]);
    name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') && index >= -1
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.b.len() - self.p
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], BinaryError> {
        if n > self.remaining() {
            return Err(BinaryError::Truncated { at: self.p });
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, BinaryError> {
        Ok(self.take(1)?[0])
    }

    fn i32(&mut self) -> Result<i32, BinaryError> {
        let s = self.take(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn i64(&mut self) -> Result<i64, BinaryError> {
        let s = self.take(8)?;
        Ok(i64::from_le_bytes(s.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, BinaryError> {
        let s = self.take(4)?;
        Ok(f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn f64(&mut self) -> Result<f64, BinaryError> {
        let s = self.take(8)?;
        Ok(f64::from_le_bytes(s.try_into().unwrap()))
    }

    fn string(&mut self) -> Result<String, BinaryError> {
        let at = self.p;
        let mut len: usize = 0;
        let mut shift = 0;
        loop {
            let byte = self.u8()?;
            len |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                return Err(BinaryError::BadString { at });
            }
        }
        let bytes = self.take(len).map_err(|_| BinaryError::BadString { at })?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    fn count(&mut self, min_size: usize) -> Result<usize, BinaryError> {
        let at = self.p;
        let n = self.i32()?;
        if n < 0 || (n as usize).saturating_mul(min_size) > self.remaining() {
            return Err(BinaryError::BadCount { at });
        }
        Ok(n as usize)
    }

    fn chunk(&mut self, depth: usize) -> Result<Chunk, BinaryError> {
        if depth > MAX_DEPTH {
            return Err(BinaryError::TooDeep);
        }
        let name = self.string()?;
        let index = self.i32()?;
        // Smallest item: 1-byte name + index + type + 1-byte value; smallest chunk: 1 + 3 ints.
        let item_count = self.count(10)?;
        let chunk_count = self.count(13)?;
        let mut items = Vec::with_capacity(item_count.min(4096));
        for _ in 0..item_count {
            items.push(self.item()?);
        }
        let mut chunks = Vec::with_capacity(chunk_count.min(4096));
        for _ in 0..chunk_count {
            chunks.push(self.chunk(depth + 1)?);
        }
        Ok(Chunk { name, index: (index >= 0).then_some(index), items, chunks })
    }

    fn item(&mut self) -> Result<Item, BinaryError> {
        let name = self.string()?;
        let index = self.i32()?;
        let at = self.p;
        let code = self.i32()?;
        let value = match code {
            types::BOOL => Value::Bool(self.u8()? != 0),
            types::BYTE => Value::Int(self.u8()? as i64),
            types::INT32 => Value::Int(self.i32()? as i64),
            types::INT64 => Value::Int(self.i64()?),
            types::SINGLE => Value::Float(self.f32()? as f64),
            types::DOUBLE => Value::Float(self.f64()?),
            types::DATE => Value::Date(self.i64()?),
            types::GUID => {
                let b: [u8; 16] = self.take(16)?.try_into().unwrap();
                Value::Guid(guid_from_dotnet_bytes(&b))
            }
            types::STRING => Value::Str(self.string()?),
            types::BYTE_ARRAY => {
                let n = self.count(1)?;
                Value::Bytes(self.take(n)?.to_vec())
            }
            types::BITMAP => {
                let n = self.count(1)?;
                self.take(n)?;
                Value::Other(code)
            }
            types::DOUBLE_ARRAY => {
                let n = self.count(8)?;
                self.take(n * 8)?;
                Value::Other(code)
            }
            types::VERSION => Value::Version(self.i32()?, self.i32()?, self.i32()?),
            other => match types::fixed_size(other) {
                Some(n) => {
                    self.take(n)?;
                    Value::Other(other)
                }
                None => return Err(BinaryError::UnknownType { at, code: other }),
            },
        };
        Ok(Item { name, index: (index >= 0).then_some(index), value })
    }
}
