//! Round trips against the `zstd` command.

use super::*;

/// Round trips through the `zstd` command when it is installed (levels 1, 3, 19, long data).
#[test]
fn against_zstd_command() {
    let mut data = Vec::new();
    let mut x = 12345u32;
    for i in 0..400_000u32 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        let w = ["shader", "vertex", "pixel", "BNSH", "\0\0\0", "grsc"][(x >> 16) as usize % 6];
        data.extend_from_slice(w.as_bytes());
        if i % 7 == 0 {
            data.push((x >> 8) as u8);
        }
    }
    for level in ["-1", "-3", "-19"] {
        let Ok(mut child) = std::process::Command::new("zstd")
            .args([level, "-c", "-q"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        if let Some(mut i) = child.stdin.take() {
            use std::io::Write;
            let _ = i.write_all(&data);
        }
        let Ok(o) = child.wait_with_output() else {
            return;
        };
        let got = decompress(&o.stdout);
        assert!(
            got.as_ref().is_ok_and(|g| *g == data),
            "level {level}: {:?}",
            got.err()
        );
    }
}

/// A frame of raw blocks and RLE blocks, built by hand (a header with a 1 MiB window, no checksum).
fn raw_frame(parts: &[(bool, Vec<u8>, usize)]) -> Vec<u8> {
    let mut f = vec![0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x50];
    for (i, (rle, data, n)) in parts.iter().enumerate() {
        let last = u32::from(i + 1 == parts.len());
        if *rle {
            f.extend_from_slice(&(((*n as u32) << 3) | 2 | last).to_le_bytes()[..3]);
            f.push(data[0]);
        } else {
            f.extend_from_slice(&(((data.len() as u32) << 3) | last).to_le_bytes()[..3]);
            f.extend_from_slice(data);
        }
    }
    f
}

#[test]
fn stream_equals_one_shot_and_obeys_the_limit() {
    let frame = raw_frame(&[
        (false, vec![1, 2, 3, 4], 0),
        (true, vec![9], 100_000),
        (false, vec![5; 10], 0),
    ]);
    let mut two = frame.clone();
    two.extend_from_slice(&frame);
    let all = decompress(&two).unwrap();
    assert_eq!(all.len(), 2 * 100_014);
    assert_eq!(decompress_limited(&two, all.len() as u64).unwrap(), all);
    assert!(decompress_limited(&two, all.len() as u64 - 1).is_err());
    // a truncated frame, an unknown magic and a block of 129 KiB are errors
    assert!(decompress_limited(&frame[..frame.len() - 3], 1 << 20).is_err());
    assert!(decompress_limited(&[1, 2, 3, 4, 5], 1 << 20).is_err());
    let big = raw_frame(&[(true, vec![0], 129 << 10)]);
    assert!(decompress_limited(&big, 1 << 30).is_err());
}

/// Streams a long input through the sink in pieces: output larger than the window plus slack leaves in parts.
#[test]
fn stream_flushes_in_pieces_and_matches_the_command() {
    let mut data = Vec::new();
    let mut x = 777u32;
    for _ in 0..3_000_000u32 {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        data.push(b"abcdefgh"[(x >> 20) as usize % 8]);
    }
    let Ok(mut child) = std::process::Command::new("zstd")
        .args(["-3", "-c", "-q", "--no-check", "--zstd=wlog=18"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    else {
        return;
    };
    let mut input = child.stdin.take().unwrap();
    let data2 = data.clone();
    let w = std::thread::spawn(move || {
        use std::io::Write;
        let _ = input.write_all(&data2);
    });
    let z = child.wait_with_output().unwrap().stdout;
    w.join().unwrap();
    let mut at = 0usize;
    let mut inp = |buf: &mut [u8]| -> R<()> {
        buf.copy_from_slice(z.get(at..at + buf.len()).ok_or_else(|| err("end"))?);
        at += buf.len();
        Ok(())
    };
    let (mut got, mut pieces) = (Vec::new(), 0);
    decompress_stream(&mut inp, z.len() as u64, data.len() as u64, &mut |b| {
        got.extend_from_slice(b);
        pieces += 1;
        Ok(())
    })
    .unwrap();
    assert!(got == data);
    assert!(pieces > 1, "the output went out in {pieces} piece");
}
