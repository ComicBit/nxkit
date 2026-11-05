//! The reuse of the previous IPA (`pack_ipa.py`'s `<out>.idx`): the index of the last pack (source metadata and the
//! compression level, bound to the IPA's size and time), the central directory of the old IPA, the kept prefix (an APFS
//! clone of the old file cut after the leading entries that did not change) and the copy of already compressed files.

use std::collections::HashMap;
use std::fs::{self, File};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::Path;

use crate::ipa_zip::{DIR_MODE, Entry, Head, L32, crc32, header, is_dir, is_link, is_reg};
use nxb_pyjson::Value;

/// `IDX_VERSION` and the deflate level of the index.
const VERSION: i64 = 3;
const LEVEL: i64 = 6;

/// A regular file's fingerprint: size, mtime ns, ctime ns, inode and the deflate flag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fp(pub u64, pub i64, pub i64, pub u64, pub bool);

impl Fp {
    pub(crate) fn of(path: &Path, deflate: bool) -> Option<Fp> {
        let m = fs::metadata(path).ok()?;
        Some(Fp(
            m.size(),
            m.mtime() * 1_000_000_000 + m.mtime_nsec(),
            m.ctime() * 1_000_000_000 + m.ctime_nsec(),
            m.ino(),
            deflate,
        ))
    }
    fn value(&self) -> Value {
        Value::List(vec![
            Value::Int(self.0 as i64),
            Value::Int(self.1),
            Value::Int(self.2),
            Value::Int(self.3 as i64),
            Value::Bool(self.4),
        ])
    }
    fn from(v: &Value) -> Option<Fp> {
        let l = v.as_list()?;
        let b = |i: usize| match l.get(i)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        };
        Some(Fp(
            l.first()?.as_i64()? as u64,
            l.get(1)?.as_i64()?,
            l.get(2)?.as_i64()?,
            l.get(3)?.as_i64()? as u64,
            b(4)?,
        ))
    }
}

/// One central directory record of the old IPA.
#[derive(Clone, Copy)]
pub(crate) struct Info {
    pub off: u64,
    pub method: u16,
    pub crc: u32,
    pub csize: u64,
    pub usize_: u64,
}

/// The loaded index and the old IPA.
pub(crate) struct Old {
    pub file: File,
    pub size: u64,
    pub files: HashMap<String, Fp>,
    pub infos: HashMap<String, Info>,
}

/// The text of `<out>.idx`.
pub(crate) fn index_text(ipa: (u64, i64), files: &[(&str, Fp)]) -> String {
    let f: Vec<(String, Value)> = files
        .iter()
        .map(|(n, p)| (n.to_string(), p.value()))
        .collect();
    ascii(&nxb_pyjson::dumps(&Value::obj([
        ("version", Value::Int(VERSION)),
        ("level", Value::Int(LEVEL)),
        ("zlib", Value::str(nxb_sys::zlib::version())),
        (
            "ipa",
            Value::List(vec![Value::Int(ipa.0 as i64), Value::Int(ipa.1)]),
        ),
        ("files", Value::Object(f)),
    ])))
}

/// `ensure_ascii=True` on the text of a document: non-ASCII characters (only inside strings) as `\uXXXX`.
fn ascii(t: &str) -> String {
    let mut o = String::with_capacity(t.len());
    for c in t.chars() {
        if c.is_ascii() {
            o.push(c);
        } else {
            let mut b = [0u16; 2];
            for u in c.encode_utf16(&mut b) {
                o.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    o
}

fn le(b: &[u8], o: usize, n: usize) -> u64 {
    b.get(o..o + n).map_or(0, |s| {
        s.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64)
    })
}

/// The central directory of a zip as `zipfile` reads it (zip64 aware).
fn central(f: &File, size: u64) -> Option<HashMap<String, Info>> {
    let tail_len = size.min(22 + 20 + 56 + 65535 + 100);
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact_at(&mut tail, size - tail_len).ok()?;
    let e = tail.windows(4).rposition(|w| w == [0x50, 0x4b, 5, 6])?;
    let (mut n, mut cd_size, mut cd_off) = (
        le(&tail, e + 10, 2),
        le(&tail, e + 12, 4),
        le(&tail, e + 16, 4),
    );
    if (n == 0xFFFF || cd_size == L32 || cd_off == L32)
        && e >= 20
        && le(&tail, e - 20, 4) == 0x0706_4b50
    {
        let z = le(&tail, e - 20 + 8, 8);
        let mut r = [0u8; 56];
        f.read_exact_at(&mut r, z).ok()?;
        (n, cd_size, cd_off) = (le(&r, 32, 8), le(&r, 40, 8), le(&r, 48, 8));
    }
    let mut cd = vec![0u8; cd_size as usize];
    f.read_exact_at(&mut cd, cd_off).ok()?;
    let mut out = HashMap::new();
    let mut p = 0usize;
    for _ in 0..n {
        if le(&cd, p, 4) != 0x0201_4b50 {
            return None;
        }
        let (method, crc) = (le(&cd, p + 10, 2) as u16, le(&cd, p + 16, 4) as u32);
        let (mut cs, mut us, mut off) =
            (le(&cd, p + 20, 4), le(&cd, p + 24, 4), le(&cd, p + 42, 4));
        let (nl, el, cl) = (
            le(&cd, p + 28, 2) as usize,
            le(&cd, p + 30, 2) as usize,
            le(&cd, p + 32, 2) as usize,
        );
        let name = String::from_utf8_lossy(cd.get(p + 46..p + 46 + nl)?).into_owned();
        let ex = cd.get(p + 46 + nl..p + 46 + nl + el)?;
        let mut q = 0;
        while q + 4 <= ex.len() {
            let (id, len) = (le(ex, q, 2), le(ex, q + 2, 2) as usize);
            if id == 1 {
                let mut k = q + 4;
                for v in [&mut us, &mut cs, &mut off] {
                    if *v == L32 {
                        *v = le(ex, k, 8);
                        k += 8;
                    }
                }
            }
            q += 4 + len;
        }
        out.insert(
            name,
            Info {
                off,
                method,
                crc,
                csize: cs,
                usize_: us,
            },
        );
        p += 46 + nl + el + cl;
    }
    Some(out)
}

/// `load_index`: the old IPA and its index when `<out>.idx` describes the file there now and this deflate level.
pub(crate) fn load(out: &Path) -> Option<Old> {
    let mut idx_path = out.as_os_str().to_owned();
    idx_path.push(".idx");
    let v = nxb_pyjson::parse(&fs::read_to_string(idx_path).ok()?).ok()?;
    let file = File::open(out).ok()?;
    let m = file.metadata().ok()?;
    let ipa = v.get("ipa")?.as_list()?;
    let ok = v.get("version")?.as_i64()? == VERSION
        && v.get("level")?.as_i64()? == LEVEL
        && v.get("zlib")?.as_str()? == nxb_sys::zlib::version()
        && ipa.first()?.as_i64()? == m.size() as i64
        && ipa.get(1)?.as_i64()? == m.mtime() * 1_000_000_000 + m.mtime_nsec();
    if !ok {
        return None;
    }
    let Value::Object(fs_) = v.get("files")? else {
        return None;
    };
    let files = fs_
        .iter()
        .map(|(k, v)| Some((k.clone(), Fp::from(v)?)))
        .collect::<Option<HashMap<_, _>>>()?;
    let infos = central(&file, m.size())?;
    Some(Old {
        file,
        size: m.size(),
        files,
        infos,
    })
}

/// `match`: the new local header (and its fields) of an entry whose bytes in the old IPA are the same.
pub(crate) fn matched(
    e: &Entry,
    fp: Option<&Fp>,
    old: &Old,
    apfs: &[bool],
) -> Option<(Vec<u8>, Head, Info)> {
    let info = *old.infos.get(&e.name)?;
    let mode = e.st.map_or(DIR_MODE, |s| s.0);
    if !is_dir(mode) && !apfs[e.root] {
        return None;
    }
    let mut data = vec![];
    let (method, crc, csize, usize_);
    if is_reg(mode) {
        if old.files.get(&e.name) != fp
            || fp.is_none()
            || info.usize_ != e.st?.2
            || !matches!(info.method, 0 | 8)
        {
            return None;
        }
        (method, crc, csize, usize_) = (info.method, info.crc, info.csize, info.usize_);
    } else {
        if is_link(mode) {
            data = fs::read_link(e.path.as_ref()?)
                .ok()?
                .as_os_str()
                .as_encoded_bytes()
                .to_vec();
        }
        (method, crc, csize, usize_) = (0, crc32(&data), data.len() as u64, data.len() as u64);
        if (info.method, info.crc, info.csize, info.usize_) != (method, crc, csize, usize_) {
            return None;
        }
    }
    let (h, head) = header(e, method, crc, csize, usize_, usize_ >= L32 || csize >= L32);
    if info.off + h.len() as u64 + csize > old.size {
        return None;
    }
    let mut got = vec![0u8; h.len() + data.len()];
    old.file.read_exact_at(&mut got, info.off).ok()?;
    if got[..h.len()] != h[..] || got[h.len()..] != data[..] {
        return None;
    }
    Some((h, head, info))
}

/// `prefix`: the leading entries unchanged and contiguous in the old IPA, and their length in bytes.
pub(crate) fn prefix(
    es: &[Entry],
    fps: &[Option<Fp>],
    old: &Old,
    apfs: &[bool],
) -> (Vec<(usize, Head, Info)>, u64) {
    let (mut keep, mut pos) = (vec![], 0u64);
    for (i, e) in es.iter().enumerate() {
        let Some(info) = old.infos.get(&e.name) else {
            break;
        };
        if info.off != pos {
            break;
        }
        let Some((h, head, info)) = matched(e, fps[i].as_ref(), old, apfs) else {
            break;
        };
        pos += h.len() as u64 + info.csize;
        keep.push((i, head, info));
    }
    (keep, pos)
}
