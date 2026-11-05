//! Checked reads for the readers: a read past the end is the Python's `struct.error` (the file is unreadable).

use crate::SplitError;

/// A `red()` of the Python as an error.
pub(crate) fn red<T>(m: impl Into<String>) -> Result<T, SplitError> {
    Err(SplitError::Red(m.into()))
}

fn short() -> SplitError {
    SplitError::Red("struct.error: unpack_from requires a buffer of more bytes".into())
}

pub(crate) fn u16le(d: &[u8], o: usize) -> Result<usize, SplitError> {
    let b = d.get(o..o.wrapping_add(2)).ok_or_else(short)?;
    Ok(usize::from(u16::from_le_bytes([b[0], b[1]])))
}

pub(crate) fn u32le(d: &[u8], o: usize) -> Result<usize, SplitError> {
    let b = d.get(o..o.wrapping_add(4)).ok_or_else(short)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

pub(crate) fn u32be(d: &[u8], o: usize) -> Result<usize, SplitError> {
    let b = d.get(o..o.wrapping_add(4)).ok_or_else(short)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

pub(crate) fn u64le(d: &[u8], o: usize) -> Result<u64, SplitError> {
    let b = d.get(o..o.wrapping_add(8)).ok_or_else(short)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

pub(crate) fn u64be(d: &[u8], o: usize) -> Result<u64, SplitError> {
    let b = d.get(o..o.wrapping_add(8)).ok_or_else(short)?;
    Ok(u64::from_be_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// `d[a:b]` as Python slices (clamped).
pub(crate) fn sl(d: &[u8], a: usize, b: usize) -> &[u8] {
    let a = a.min(d.len());
    &d[a..b.clamp(a, d.len())]
}

/// `zlib.decompress(d)`: the stream must be whole and its checksum right (bytes after it are ignored).
pub(crate) fn zlib_all(d: &[u8], what: &str) -> Result<Vec<u8>, SplitError> {
    crate::zlibx::zlib_stream(d, 1 << 31).ok_or_else(|| {
        SplitError::Red(format!(
            "zlib.error: {what}: damaged or truncated zlib stream"
        ))
    })
}
