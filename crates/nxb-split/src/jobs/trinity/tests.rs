use super::*;
use crate::src::Mem;

/// A small FlatBuffers writer: tables with their vtable in front, offsets patched once the target is placed.
struct B(Vec<u8>);

impl B {
    fn table(&mut self, fields: &[Option<Vec<u8>>]) -> (u64, Vec<u64>) {
        let vt = self.0.len();
        self.0
            .extend_from_slice(&(4 + 2 * fields.len() as u16).to_le_bytes());
        self.0.extend_from_slice(&0u16.to_le_bytes());
        self.0.resize(vt + 4 + 2 * fields.len(), 0);
        let t = self.0.len();
        self.0.extend_from_slice(&((t - vt) as i32).to_le_bytes());
        let mut at = Vec::new();
        for (i, f) in fields.iter().enumerate() {
            let p = self.0.len();
            match f {
                Some(b) => {
                    self.0.extend_from_slice(b);
                    let o = (p - t) as u16;
                    self.0[vt + 4 + 2 * i..vt + 6 + 2 * i].copy_from_slice(&o.to_le_bytes());
                }
                None => {}
            }
            at.push(p as u64);
        }
        (t as u64, at)
    }
    fn at(&mut self, at: u64, target: u64) {
        let a = at as usize;
        self.0[a..a + 4].copy_from_slice(&((target - at) as u32).to_le_bytes());
    }
    fn vec_u64(&mut self, v: &[u64]) -> u64 {
        let p = self.0.len() as u64;
        self.0.extend_from_slice(&(v.len() as u32).to_le_bytes());
        for x in v {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        p
    }
    /// A vector of offsets, patched by the caller: (vector, slots).
    fn vec_refs(&mut self, n: usize) -> (u64, Vec<u64>) {
        let p = self.0.len() as u64;
        self.0.extend_from_slice(&(n as u32).to_le_bytes());
        let slots = (0..n).map(|k| p + 4 + 4 * k as u64).collect();
        self.0.resize(self.0.len() + 4 * n, 0);
        (p, slots)
    }
    fn bytes(&mut self, b: &[u8]) -> u64 {
        let p = self.0.len() as u64;
        self.0.extend_from_slice(&(b.len() as u32).to_le_bytes());
        self.0.extend_from_slice(b);
        p
    }
}

fn off() -> Option<Vec<u8>> {
    Some(vec![0; 4])
}

/// The descriptor: two files (hashes 0x11, 0x22) in packs 0 and 1, named "a" and "b", 10 and 20 bytes.
fn descriptor() -> Vec<u8> {
    let mut b = B(vec![0; 4]);
    let (fr, f) = b.table(&[off(), off(), off(), off()]);
    b.at(0, fr);
    let h = b.vec_u64(&[0x11, 0x22]);
    b.at(f[0], h);
    let (nv, ns) = b.vec_refs(2);
    b.at(f[1], nv);
    for (s, n) in ns.iter().zip(["a", "b"]) {
        let p = b.bytes(n.as_bytes());
        b.at(*s, p);
    }
    let (iv, is) = b.vec_refs(2);
    b.at(f[2], iv);
    for (s, k) in is.iter().zip([0u64, 1]) {
        let (t, _) = b.table(&[Some(k.to_le_bytes().to_vec())]);
        b.at(*s, t);
    }
    let (pv, ps) = b.vec_refs(2);
    b.at(f[3], pv);
    for (s, k) in ps.iter().zip([10u64, 20]) {
        let (t, _) = b.table(&[Some(k.to_le_bytes().to_vec())]);
        b.at(*s, t);
    }
    b.0
}

/// The file system: pack "b" first in the file, holding one stored file and one of compression `comp`.
fn file_system(comp: i8) -> Vec<u8> {
    let mut b = B(ONEPACK.to_vec());
    b.0.extend_from_slice(&16u64.to_le_bytes());
    b.0.extend_from_slice(&[0; 4]);
    let (sr, sf) = b.table(&[off(), off()]);
    b.at(16, sr);
    let ov = b.0.len() as u64;
    b.0.extend_from_slice(&2u32.to_le_bytes());
    let slots = [ov + 4, ov + 12];
    b.0.resize(b.0.len() + 16, 0);
    b.at(sf[1], ov);
    for (k, h) in [0x22u64, 0x11].into_iter().enumerate() {
        let pos = b.0.len() as u64;
        b.0[slots[k] as usize..slots[k] as usize + 8].copy_from_slice(&pos.to_le_bytes());
        b.0.extend_from_slice(&[0; 4]);
        let (pr, pf) = b.table(&[off(), off()]);
        b.at(pos, pr);
        let hv = b.vec_u64(&[h, 0x33]);
        b.at(pf[0], hv);
        let (fv, fs) = b.vec_refs(2);
        b.at(pf[1], fv);
        for (j, s) in fs.iter().enumerate() {
            let c = if j == 1 { comp } else { 0 };
            let n: u64 = if c == 0 { 5 } else { 9 };
            let (t, tf) = b.table(&[
                None,
                Some(vec![c as u8]),
                None,
                Some(n.to_le_bytes().to_vec()),
                off(),
            ]);
            b.at(*s, t);
            let d = b.bytes(b"hello");
            b.at(tf[4], d);
        }
    }
    b.0
}

fn files(fs: Vec<u8>, fd: Vec<u8>) -> Vec<File> {
    let own = |v: Vec<u8>| -> Arc<dyn Src> { Arc::new(Owned(v)) };
    vec![
        File {
            rel: "arc/data.trpfs".into(),
            src: own(fs),
            live: true,
        },
        File {
            rel: "arc/data.trpfd".into(),
            src: own(fd),
            live: true,
        },
    ]
}

struct Owned(Vec<u8>);
impl Src for Owned {
    fn bytes(&self) -> Option<&[u8]> {
        Some(&self.0)
    }
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&self, off: u64, buf: &mut [u8]) -> R<()> {
        Mem(&self.0).read_at(off, buf)
    }
}

#[test]
fn packs_are_named_through_their_first_file() {
    let fs = file_system(0);
    let fd = descriptor();
    let p = packs(&Mem(&fs), &fd, "arc/data.trpfs").unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].1.as_str(), p[0].2), ("b", 20));
    assert_eq!((p[1].1.as_str(), p[1].2), ("a", 10));
    assert!(p[0].0 < p[1].0);
}

#[test]
fn plan_takes_both_files_and_scans_stored_files() {
    let f = files(file_system(0), descriptor());
    let mut p = Plan::default();
    plan(&f, &mut p).unwrap();
    assert_eq!(p.taken.len(), 2);
    // a step of 30 / 64: one job per pack
    assert_eq!(p.jobs.len(), 2);
    let ctx = Ctx {
        files: &f,
        dicts: &crate::zstd::Dicts::default(),
    };
    let r = (p.jobs[0].run)(&ctx, &mut Vec::new()).unwrap();
    assert!(r.found.is_empty());
}

#[test]
fn unknown_compression_is_red() {
    let f = files(file_system(7), descriptor());
    let mut p = Plan::default();
    plan(&f, &mut p).unwrap();
    let ctx = Ctx {
        files: &f,
        dicts: &crate::zstd::Dicts::default(),
    };
    let Err(crate::SplitError::Red(m)) = (p.jobs[0].run)(&ctx, &mut Vec::new()) else {
        panic!("expected red");
    };
    assert_eq!(
        m,
        "/arc/data.trpfs#b: file 1 has compression 7 (5 → 9 bytes), which is not known"
    );
}

#[test]
fn a_pack_without_a_listed_file_is_red() {
    let mut fd = descriptor();
    // the first hash of the descriptor no longer names pack "b"'s first file
    let at = fd.windows(8).position(|w| w == 0x22u64.to_le_bytes()).unwrap();
    fd[at] = 0x44;
    let e = packs(&Mem(&file_system(0)), &fd, "arc/data.trpfs").unwrap_err();
    assert!(e.to_string().contains("has no file the descriptor lists"));
}
