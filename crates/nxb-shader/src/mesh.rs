//! The facts of an NVN2 task or mesh shader (port of `dmsh_meshstage.info`, `memory` and `own_shared`). The job of such
//! a shader is a compute job with its SPH taken off. It carries these facts as `mesh_stage` and the stage memory they
//! imply as `stage_memory`, `local` and `shared`. `nxb-xlate` checks the job against its own layout of the same memory.

use crate::job::JobError;
use nxb_pyjson::Value;

/// The workgroup number is word 0 of the stage memory and the payload input starts at +0x10.
const INPUT: i64 = 0x10;
/// The vertices and primitives of one attribute block.
const BLOCK: i64 = 32;
/// The threadgroup memory of a Metal function.
pub(crate) const MAX_SHARED: i64 = 0x8000;
/// Opcodes of shared memory access: LDS, STS, ATOMS and friends.
const SHARED_OPCODES: [u16; 6] = [0x984, 0x388, 0x988, 0x38c, 0x98c, 0xf8c];
/// The reason of a refused job.
pub(crate) const REFUSED: &str = "shared memory of its own, or stage memory past Metal's limit";

/// An output component of an SPH: its attribute (`None` for the position) and its index.
type Comp = (Option<i64>, i64);

/// What the control section and the SPHs say about a task or mesh shader.
pub(crate) struct Facts {
    kind: &'static str,
    threads: i64,
    payload: i64,
    task: bool,
    max_v: i64,
    max_p: i64,
    vertex: Vec<Comp>,
    primitive: Vec<Comp>,
}

fn u16_at(b: &[u8], o: usize) -> Option<i64> {
    b.get(o..o + 2)
        .map(|s| i64::from(u16::from_le_bytes([s[0], s[1]])))
}

fn u32_at(b: &[u8], o: usize) -> Option<usize> {
    b.get(o..o + 4)
        .and_then(|s| usize::try_from(u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok())
}

/// The components an SPH enables in its output map, in the order of the attribute area: the position's (byte 0x35,
/// bits 4 to 7), then the generic attributes' (bytes 0x36 on, a nibble for each attribute).
fn sph_outputs(sph: &[u8]) -> Option<Vec<Comp>> {
    let b = |o: usize| sph.get(o).copied();
    let mut out: Vec<Comp> = (0..4i64)
        .filter(|c| b(0x35).is_some_and(|v| v >> (4 + c) & 1 != 0))
        .map(|c| (None, c))
        .collect();
    b(0x35)?;
    for n in 0..32i64 {
        let v = b(0x36 + (n / 2) as usize)?;
        out.extend(
            (0..4i64)
                .filter(|c| v >> (4 * (n % 2) + c) & 1 != 0)
                .map(|c| (Some(n), c)),
        );
    }
    Some(out)
}

fn area(a: &[u8], at: usize) -> &[u8] {
    a.get(at..).map_or(&[], |s| &s[..s.len().min(0x80)])
}

/// The facts of a shader from its control section and its whole code area (the SPH, the instructions and the second
/// SPH of a mesh shader).
pub(crate) fn info(key: &str, ctl: &[u8], code_area: &[u8]) -> Result<Facts, JobError> {
    let bad = |why: &str| JobError(format!("{key}: {why}"));
    let kind = match ctl.get(0x56) {
        Some(6) => "ts",
        Some(7) => "ms",
        _ => return Err(bad("the control section names no task or mesh stage")),
    };
    let short = || bad("the control section of a task or mesh shader is too short");
    let mut f = Facts {
        kind,
        threads: u16_at(ctl, 0x58).ok_or_else(short)?,
        payload: u16_at(ctl, 0x5a).ok_or_else(short)?,
        task: true,
        max_v: 0,
        max_p: 0,
        vertex: Vec::new(),
        primitive: Vec::new(),
    };
    if kind == "ms" {
        f.task = ctl.get(0x5e).ok_or_else(short)? != &0;
        f.max_v = u16_at(ctl, 0x60).ok_or_else(short)?;
        f.max_p = u16_at(ctl, 0x62).ok_or_else(short)?;
        let incomplete = || bad("the SPH of a mesh shader is incomplete");
        f.vertex = sph_outputs(area(code_area, 0)).ok_or_else(incomplete)?;
        if ctl.get(0x68).ok_or_else(short)? & 0x80 != 0 {
            let at = u32_at(ctl, 0x64).ok_or_else(short)?;
            if at != 0 {
                let prim = sph_outputs(area(code_area, at)).ok_or_else(incomplete)?;
                f.primitive = prim.into_iter().filter(|c| c.0.is_some()).collect();
            }
        }
    }
    Ok(f)
}

fn align(v: i64, to: i64) -> i64 {
    (v + to - 1) & !(to - 1)
}

impl Facts {
    /// The offsets of the stage memory (the workgroup number, the input, the outputs, the primitive bytes) and its size
    /// in bytes.
    pub(crate) fn memory(&self) -> ([i64; 4], i64) {
        let out = INPUT + align(self.payload, 16);
        if self.kind == "ts" {
            return (
                [0, INPUT, out, out + self.payload],
                out + align(self.payload, 4),
            );
        }
        let blocks = (self.max_v.max(self.max_p).max(1) + BLOCK - 1) / BLOCK;
        let comps = (self.vertex.len() + self.primitive.len()).max(1) as i64;
        let prim = out + blocks * comps * BLOCK * 4;
        // The index bytes are reserved for whole blocks of primitives, as `nxb_xlate::msl::mesh::memory` does.
        let p_blocks = (self.max_p.max(1) + BLOCK - 1) / BLOCK;
        (
            [0, INPUT, out, prim],
            prim + align(4 + 3 * p_blocks * BLOCK, 4),
        )
    }

    /// The threads of a workgroup.
    pub(crate) fn threads(&self) -> i64 {
        self.threads
    }

    /// The object of the job's `mesh_stage`, with the reason when the build refuses the shader.
    pub(crate) fn to_value(&self, refused: bool) -> Value {
        let comps = |l: &[Comp]| {
            Value::List(
                l.iter()
                    .map(|(a, c)| {
                        Value::List(vec![a.map_or(Value::Null, Value::Int), Value::Int(*c)])
                    })
                    .collect(),
            )
        };
        let mut v = Value::obj([
            ("kind", Value::str(self.kind)),
            ("threads", Value::Int(self.threads)),
            ("payload", Value::Int(self.payload)),
        ]);
        if self.kind == "ms" {
            v.set("task", Value::Bool(self.task));
            v.set("max_v", Value::Int(self.max_v));
            v.set("max_p", Value::Int(self.max_p));
            v.set("vertex", comps(&self.vertex));
            v.set("primitive", comps(&self.primitive));
        }
        if refused {
            v.set("refused", Value::str(REFUSED));
        }
        v
    }
}

/// Whether the instructions use shared memory of their own. It would lie over the stage memory, and the control
/// section does not give its size.
pub(crate) fn own_shared(code: &[u8]) -> bool {
    code.as_chunks::<16>().0.iter().any(|i| {
        let op = u16::from_le_bytes([i[0], i[1]]) & 0xfff;
        SHARED_OPCODES.contains(&op)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctl(kind: u8, threads: u16, payload: u16) -> Vec<u8> {
        let mut c = vec![0u8; 0x70];
        c[0x56] = kind;
        c[0x58..0x5a].copy_from_slice(&threads.to_le_bytes());
        c[0x5a..0x5c].copy_from_slice(&payload.to_le_bytes());
        c
    }

    #[test]
    fn a_task_shader_has_its_payload_as_output() {
        let f = info("k", &ctl(6, 32, 20), &[]).expect("task");
        let (lay, size) = f.memory();
        assert_eq!(lay, [0, 0x10, 0x30, 0x44]);
        assert_eq!(size, 0x30 + 20);
        assert_eq!(f.threads(), 32);
        let v = f.to_value(false);
        assert_eq!(v.get("kind"), Some(&Value::str("ts")));
        assert_eq!(v.get("task"), None);
    }

    #[test]
    fn a_mesh_shader_reads_both_sphs() {
        let mut c = ctl(7, 32, 0);
        c[0x5e] = 1;
        c[0x60..0x62].copy_from_slice(&6u16.to_le_bytes());
        c[0x62..0x64].copy_from_slice(&2u16.to_le_bytes());
        c[0x64..0x68].copy_from_slice(&0x80u32.to_le_bytes());
        c[0x68] = 0x80;
        let mut area = vec![0u8; 0x100];
        area[0x35] = 0xf0;
        area[0x36] = 0x03;
        area[0x80 + 0x35] = 0x00;
        area[0x80 + 0x37] = 0x01;
        let f = info("k", &c, &area).expect("mesh");
        assert_eq!(f.vertex.len(), 6);
        assert_eq!(f.vertex[4], (Some(0), 0));
        assert_eq!(f.primitive, [(Some(2), 0)]);
        let (lay, size) = f.memory();
        assert_eq!(lay, [0, 0x10, 0x10, 0x10 + 32 * 7 * 4]);
        assert_eq!(size, lay[3] + 4 + 3 * 32);
        let v = f.to_value(true);
        assert_eq!(v.get("max_v"), Some(&Value::Int(6)));
        assert_eq!(v.get("refused"), Some(&Value::str(REFUSED)));
    }

    #[test]
    fn a_short_sph_is_an_error() {
        let mut c = ctl(7, 32, 0);
        c[0x5e] = 1;
        let e = info("k", &c, &[0; 0x40]).err().expect("short");
        assert!(e.0.contains("incomplete"), "{e}");
        assert!(info("k", &ctl(5, 1, 1), &[]).is_err());
    }

    #[test]
    fn shared_memory_opcodes_are_found() {
        let mut code = vec![0u8; 32];
        assert!(!own_shared(&code));
        code[16..18].copy_from_slice(&0x984u16.to_le_bytes());
        assert!(own_shared(&code));
    }
}
