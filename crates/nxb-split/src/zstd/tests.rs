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

/// The one-shot decoder stops at its limit: RLE blocks that expand past it are an error, not an allocation.
#[test]
fn one_shot_obeys_the_limit() {
    let rle: Vec<(bool, Vec<u8>, usize)> = (0..64).map(|i| (true, vec![7], 1 << 17 | i)).collect();
    let frame = raw_frame(&rle);
    let n = decompress(&frame).unwrap().len();
    assert_eq!(decompress_max(&frame, n).unwrap().len(), n);
    assert!(matches!(
        decompress_max(&frame, n - 1),
        Err(SplitError::Format(_))
    ));
    // the default limit gives `Unsupported` (the reference step decodes such files)
    let r = frames(&frame, 1 << 20, &Dicts::new(), |n| {
        SplitError::Unsupported(format!("over {n}"))
    });
    assert!(matches!(r, Err(SplitError::Unsupported(_))));
}

/// Frames that name a dictionary decode with it, byte for byte (the `zstd` command trains and applies it).
#[test]
fn dictionary_frames_match_the_zstd_command() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("nxb-zstd-dict-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sample = |k: u32| -> Vec<u8> {
        let mut x = k.wrapping_mul(2654435761).wrapping_add(7);
        let mut v = Vec::new();
        for i in 0..600u32 {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            let w = [
                "shader", "vertex", "pixel", "BNSH", "grsc", "uniform", "layout",
            ][(x >> 16) as usize % 7];
            v.extend_from_slice(w.as_bytes());
            v.push(b'0' + (i % 10) as u8);
            if x.is_multiple_of(11) {
                v.extend_from_slice(&x.to_le_bytes());
            }
        }
        v
    };
    for k in 0..300 {
        std::fs::write(dir.join(format!("s{k:03}.bin")), sample(k)).unwrap();
    }
    let dict = dir.join("dict");
    let train = Command::new("zstd")
        .current_dir(&dir)
        .args(["--train", "-q", "--maxdict=16384", "-o", "dict"])
        .args((0..300).map(|k| format!("s{k:03}.bin")))
        .output();
    let Ok(t) = train else {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    assert!(t.status.success(), "{}", String::from_utf8_lossy(&t.stderr));
    let raw = std::fs::read(&dict).unwrap();
    let mut dicts = Dicts::new();
    let id = dicts.add(&raw).unwrap();
    for (n, level) in [(1000u32, "-1"), (1001, "-3"), (1002, "-19")] {
        let data = sample(n);
        std::fs::write(dir.join("in"), &data).unwrap();
        let o = Command::new("zstd")
            .current_dir(&dir)
            .args([level, "-q", "-f", "-D", "dict", "-c", "in"])
            .output()
            .unwrap();
        assert!(o.status.success());
        assert_eq!(dict_id(&o.stdout), id, "frame names the dictionary");
        let got = decompress_dict(&o.stdout, &dicts);
        assert!(
            got.as_ref().is_ok_and(|g| *g == data),
            "{level}: {:?}",
            got.err()
        );
        assert!(matches!(
            decompress(&o.stdout),
            Err(SplitError::Unsupported(_))
        ));
        let mut other = Dicts::new();
        let mut copy = raw.clone();
        copy[4..8].copy_from_slice(&(id ^ 1).to_le_bytes());
        other.add(&copy).unwrap();
        assert!(matches!(
            decompress_dict(&o.stdout, &other),
            Err(SplitError::Red(_))
        ));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
