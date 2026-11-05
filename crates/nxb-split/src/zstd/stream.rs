//! A streaming frame decoder over the block decoder: the input is read block by block, the output goes to a sink in
//! pieces, and only the window (the last bytes a match may refer to) stays in memory. For compressed NCAs (NCZ) whose
//! one frame is gigabytes long, and for blocks with a limit on the output (a frame made of RLE blocks expands a
//! thousand times).

use super::{R, State, block, err};
use crate::SplitError;

/// The largest window accepted (zstd's own default maximum for `--long` is 128 MiB, 2 GiB is the format's limit).
const MAX_WINDOW: u64 = 512 << 20;

/// Bytes of finished output kept above the window before they go to the sink.
const SLACK: usize = 1 << 20;

/// Reads exactly `buf.len()` bytes of the compressed input.
pub type Input<'a> = dyn FnMut(&mut [u8]) -> R<()> + 'a;

/// Takes finished output.
pub type Sink<'a> = dyn FnMut(&[u8]) -> R<()> + 'a;

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x))
}

/// Decodes the frames of an input of `input_len` bytes (skippable frames skipped, checksums not verified) and gives
/// the output to `sink`. More than `limit` bytes of output is an error. A frame with a dictionary or a window over
/// 512 MiB is `Unsupported`. Returns the bytes of output.
pub fn decompress_stream(
    input: &mut Input<'_>,
    input_len: u64,
    limit: u64,
    sink: &mut Sink<'_>,
) -> R<u64> {
    let mut used = 0u64;
    let mut total = 0u64;
    let take = |n: usize, used: &mut u64, input: &mut Input<'_>| -> R<Vec<u8>> {
        if used.saturating_add(n as u64) > input_len {
            return Err(err("input ends inside a frame"));
        }
        let mut v = vec![0u8; n];
        input(&mut v)?;
        *used += n as u64;
        Ok(v)
    };
    let mut out: Vec<u8> = Vec::new();
    while used < input_len {
        let m = le(&take(4, &mut used, input)?) as u32;
        if m & 0xFFFF_FFF0 == 0x184D_2A50 {
            let mut n = le(&take(4, &mut used, input)?);
            while n > 0 {
                let k = n.min(1 << 16) as usize;
                take(k, &mut used, input)?;
                n -= k as u64;
            }
            continue;
        }
        if m != 0xFD2F_B528 {
            return Err(err("unknown frame magic"));
        }
        let fhd = take(1, &mut used, input)?[0];
        let single = fhd >> 5 & 1 == 1;
        let did = [0usize, 1, 2, 4][(fhd & 3) as usize];
        let fcs = match fhd >> 6 {
            0 => usize::from(single),
            1 => 2,
            2 => 4,
            _ => 8,
        };
        let wd = if single {
            0
        } else {
            take(1, &mut used, input)?[0]
        };
        if take(did, &mut used, input)?.iter().any(|&b| b != 0) {
            return Err(SplitError::Unsupported(
                "zstd frames with a dictionary".into(),
            ));
        }
        let size = le(&take(fcs, &mut used, input)?) + if fcs == 2 { 256 } else { 0 };
        let window = if single {
            size
        } else {
            let base = 1u64 << (10 + u32::from(wd >> 3));
            base + base / 8 * u64::from(wd & 7)
        };
        if window > MAX_WINDOW {
            return Err(SplitError::Unsupported(format!(
                "zstd window of {window} bytes"
            )));
        }
        let keep = window.max(1) as usize;
        let mut st = State {
            rep: [1, 4, 8],
            ..State::default()
        };
        out.clear();
        loop {
            let h = take(3, &mut used, input)?;
            let h = h[0] as usize | (h[1] as usize) << 8 | (h[2] as usize) << 16;
            let (last, ty, bsize) = (h & 1 == 1, (h >> 1) & 3, h >> 3);
            if bsize > 1 << 17 {
                return Err(err("block larger than 128 KiB"));
            }
            match ty {
                0 => out.extend_from_slice(&take(bsize, &mut used, input)?),
                1 => {
                    let b = take(1, &mut used, input)?[0];
                    out.extend(std::iter::repeat_n(b, bsize));
                }
                2 => {
                    let src = take(bsize, &mut used, input)?;
                    block(&src, &mut st, &mut out, 0)?;
                }
                _ => return Err(err("reserved block type")),
            }
            if total + out.len() as u64 > limit {
                return Err(err("output larger than expected"));
            }
            if out.len() > keep + SLACK {
                let n = out.len() - keep;
                sink(&out[..n])?;
                total += n as u64;
                out.drain(..n);
            }
            if last {
                break;
            }
        }
        if fhd & 4 != 0 {
            take(4, &mut used, input)?;
        }
        sink(&out)?;
        total += out.len() as u64;
        out.clear();
    }
    Ok(total)
}

/// Like [`super::decompress`] with a limit on the size of the output.
pub fn decompress_limited(d: &[u8], limit: u64) -> R<Vec<u8>> {
    let mut at = 0usize;
    let mut input = |buf: &mut [u8]| -> R<()> {
        let part = d
            .get(at..at + buf.len())
            .ok_or_else(|| err("input ends inside a frame"))?;
        buf.copy_from_slice(part);
        at += buf.len();
        Ok(())
    };
    let mut out = Vec::new();
    decompress_stream(&mut input, d.len() as u64, limit, &mut |b| {
        out.extend_from_slice(b);
        Ok(())
    })?;
    Ok(out)
}
