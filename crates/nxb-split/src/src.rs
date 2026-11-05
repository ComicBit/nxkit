//! Where the readers take bytes from: a file of an image read by ranges, or bytes in memory. The big archives are
//! read through [`Src`] (entry ranges only), every other reader gets the whole file.

use crate::SplitError;

/// A readable file.
pub trait Src: Send + Sync {
    /// Size in bytes.
    fn len(&self) -> u64;
    /// Fills `buf` from `off`.
    fn read_at(&self, off: u64, buf: &mut [u8]) -> Result<(), SplitError>;
    /// The bytes when they are in memory (no copy for the readers that take the whole file).
    fn bytes(&self) -> Option<&[u8]> {
        None
    }
    /// Whether the file is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Bytes in memory.
pub struct Mem<'a>(pub &'a [u8]);

impl Src for Mem<'_> {
    fn bytes(&self) -> Option<&[u8]> {
        Some(self.0)
    }
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&self, off: u64, buf: &mut [u8]) -> Result<(), SplitError> {
        let a = usize::try_from(off).unwrap_or(usize::MAX);
        let part = self
            .0
            .get(a..a.saturating_add(buf.len()))
            .ok_or_else(|| SplitError::Red("read past the end of the file".into()))?;
        buf.copy_from_slice(part);
        Ok(())
    }
}

impl dyn Src + '_ {
    /// `n` bytes at `off`, fewer when the file ends (as Python's `f.read`).
    pub fn read_up(&self, off: u64, n: usize) -> Result<Vec<u8>, SplitError> {
        let n = n.min(self.len().saturating_sub(off) as usize);
        let mut v = vec![0u8; n];
        self.read_at(off, &mut v)?;
        Ok(v)
    }

    /// The whole file, borrowed when it is in memory.
    pub fn whole(&self) -> Result<std::borrow::Cow<'_, [u8]>, SplitError> {
        match self.bytes() {
            Some(b) => Ok(b.into()),
            None => self.all().map(Into::into),
        }
    }

    /// The whole file.
    pub fn all(&self) -> Result<Vec<u8>, SplitError> {
        self.read_up(0, usize::try_from(self.len()).unwrap_or(usize::MAX))
    }

    /// Position of `pat` at or after `from` (read in chunks).
    pub fn find(&self, pat: &[u8], from: u64) -> Result<Option<u64>, SplitError> {
        const CH: usize = 1 << 22;
        let mut at = from;
        while at < self.len() {
            let b = self.read_up(at, CH + pat.len())?;
            if let Some(p) = crate::shaders::memfind(&b, pat) {
                return Ok(Some(at + p as u64));
            }
            if b.len() < CH + pat.len() {
                break;
            }
            at += CH as u64;
        }
        Ok(None)
    }
}
