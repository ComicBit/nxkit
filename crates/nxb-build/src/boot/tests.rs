use super::*;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nxb-boot-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).ok().expect("dir");
    d
}

fn script(d: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = d.join("host.sh");
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n"))
        .ok()
        .expect("write");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))
        .ok()
        .expect("chmod");
    p
}

fn env() -> Vec<(OsString, OsString)> {
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("X".into(), "42".into()),
    ]
}

#[test]
fn hms_formats() {
    assert_eq!(hms(0.0), "0:00");
    assert_eq!(hms(65.9), "1:05");
    assert_eq!(hms(3725.0), "1:02:05");
}

#[test]
fn boot_exit_code_and_env() {
    let d = dir("exit");
    let h = script(&d, "echo \"arg=$1 x=$X\"; pwd; exit 3");
    let log = d.join("logs/a.log");
    let r = boot_mac(&h, Path::new("/mods"), &env(), &d, &log, 10.0, &mut || {
        false
    })
    .ok()
    .expect("boot");
    assert_eq!(r.rc, Some(3));
    let t = read_log(&log);
    assert!(t.starts_with("arg=/mods x=42\n"), "{t}");
    assert!(t.contains("nxb-boot-exit"), "{t}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn boot_timeout_and_done() {
    let d = dir("time");
    let h = script(
        &d,
        "echo up; trap 'echo term; exit 0' TERM; while :; do sleep 0.05; done",
    );
    let log = d.join("b.log");
    let r = boot_mac(&h, &d, &env(), &d, &log, 0.6, &mut || false)
        .ok()
        .expect("boot");
    assert_eq!(r.rc, None);
    assert!(r.secs >= 0.6 && r.secs < 5.0, "{}", r.secs);
    let mut n = 0;
    let r = boot_mac(&h, &d, &env(), &d, &log, 30.0, &mut || {
        n += 1;
        n > 2
    })
    .ok()
    .expect("boot");
    assert_eq!(r.rc, None);
    assert!(r.secs < 5.0);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn tail_and_stopped() {
    let d = dir("tail");
    let log = d.join("c.log");
    let text: String = (1..=20).map(|i| format!("l{i}\n")).collect();
    std::fs::write(&log, text).ok().expect("w");
    assert_eq!(tail_lines(&log, 3), "l18\nl19\nl20");
    let s = host_stopped(&log, 1, 12.4);
    assert!(s.starts_with(
        "[RED] the game stopped after 12 s in the Mac host (exit 1): its shaders are incomplete"
    ));
    assert!(s.ends_with("----\nl9\nl10\nl11\nl12\nl13\nl14\nl15\nl16\nl17\nl18\nl19\nl20"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn captured_nros_needs_a_capture_row() {
    let d = dir("nros");
    assert_eq!(captured_nros(&d), None);
    std::fs::write(d.join("nro.tsv"), "# h\nnro0\tab\t1\tx.nrr\tm.nro\n")
        .ok()
        .expect("w");
    assert_eq!(captured_nros(&d), None);
    std::fs::write(d.join("nro.tsv"), "# h\nnro0\tab\t1\tx.nrr\tcapture\n")
        .ok()
        .expect("w");
    assert_eq!(captured_nros(&d), Some(d.clone()));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn rmtree_and_swap() {
    let d = dir("swap");
    rmtree(&d.join("none")).ok().expect("missing ok");
    std::fs::create_dir_all(d.join("new")).ok().expect("m");
    std::fs::write(d.join("new/f"), "1").ok().expect("w");
    std::fs::create_dir_all(d.join("old")).ok().expect("m");
    std::fs::write(d.join("old/g"), "2").ok().expect("w");
    swap_in(&d.join("new"), &d.join("old")).ok().expect("swap");
    assert!(d.join("old/f").is_file() && !d.join("old/g").exists() && !d.join("new").exists());
    let _ = std::fs::remove_dir_all(&d);
}
