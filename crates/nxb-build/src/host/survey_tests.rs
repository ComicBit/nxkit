//! The survey and the provisional profile against `build_ipa_from_nsp.py` (`survey`, `write_profile`).
//! `NXB_ORACLE_SURVEY` holds `<name>/<title ID>_survey/survey.md` written by the script for the extraction
//! `NXB_ORACLE_EXTRACTS/<name>`, `NXB_ORACLE_ENGINE` is the engine folder. The census block is taken from the script's
//! report (it is not ported), the SDK surface block compares as a set of lines (the script lists areas of equal
//! count in the order of a hash set).

use super::survey::{SurveyInput, survey};
use super::survey_profile::*;
use std::path::PathBuf;

fn block<'a>(t: &'a str, from: &str, to: &str) -> &'a str {
    let a = t.find(from).map_or(0, |i| i + from.len());
    let b = t[a..].find(to).map_or(t.len(), |i| a + i);
    &t[a..b]
}

/// The report without the order of the SDK surface block. The script cuts its area list at 40 rows and its example
/// list at 14 areas in the order of a hash set where areas tie, so the areas with the count of the last row are left
/// out of both reports, and the examples of areas the other report does not list.
fn normal(t: &str, other: &str) -> String {
    let (a, b) = (
        t.find("## Imports vs host hooks").unwrap(),
        t.find("## What it means").unwrap(),
    );
    let area = |l: &str| l.trim_start().split(": ").next().unwrap_or("").to_string();
    let example = |l: &str| l.starts_with("  ") && l.contains(": ");
    let rows = |x: &str| -> Vec<(String, String)> {
        x.lines()
            .filter(|l| l.starts_with("  ") && l.contains(" / ") && !l.contains(": "))
            .map(|l| {
                (
                    l.split_whitespace().next().unwrap_or("").to_string(),
                    l.split_whitespace().nth(1).unwrap_or("").to_string(),
                )
            })
            .collect()
    };
    let cut = |x: &str| {
        let r = rows(x);
        if r.len() >= 40 {
            r.last().map(|l| l.1.clone())
        } else {
            None
        }
    };
    let last = cut(t).or_else(|| cut(other));
    let tied: Vec<String> = rows(t)
        .into_iter()
        .chain(rows(other))
        .filter(|r| Some(&r.1) == last.as_ref())
        .map(|r| r.0)
        .collect();
    let other_areas: Vec<String> = other.lines().filter(|l| example(l)).map(area).collect();
    let mut lines: Vec<&str> = t[a..b]
        .lines()
        .filter(|l| {
            let first = l
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(':');
            !tied.iter().any(|x| x == first) && (!example(l) || other_areas.contains(&area(l)))
        })
        .collect();
    lines.sort();
    format!("{}{}\n{}", &t[..a], lines.join("\n"), &t[b..])
}

#[test]
fn survey_and_profile_equal_the_reference_script() {
    let (Some(sv), Some(ex), Some(engine)) = (
        std::env::var_os("NXB_ORACLE_SURVEY"),
        std::env::var_os("NXB_ORACLE_EXTRACTS"),
        std::env::var_os("NXB_ORACLE_ENGINE"),
    ) else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!("nxb-survey-test-{}", std::process::id()));
    let mut n = 0;
    for d in std::fs::read_dir(&sv)
        .unwrap()
        .flatten()
        .filter(|d| d.path().is_dir())
    {
        let name = d.file_name().to_string_lossy().into_owned();
        let Some(sd) = std::fs::read_dir(d.path())
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().ends_with("_survey"))
        else {
            continue;
        };
        let app = u64::from_str_radix(
            sd.file_name().to_string_lossy().trim_end_matches("_survey"),
            16,
        )
        .unwrap();
        let want = std::fs::read_to_string(sd.path().join("survey.md")).unwrap();
        let census = block(
            &want,
            "## Rewrite-site census (insn_census.py)\n\n```\n",
            "\n```\n\n## Imports vs host",
        );
        let dlc = want.contains("DLC present");
        let s = survey(&SurveyInput {
            extract: PathBuf::from(&ex).join(&name),
            app,
            fallback_name: String::new(),
            package_lines: [
                Some("- base: none (the update is the whole game)".to_string()),
                dlc.then(|| "- DLC: `dlc.nsp` (293 KB, licence only, no content)".to_string()),
            ]
            .into_iter()
            .flatten()
            .collect(),
            update: None,
            dlc,
            engine: PathBuf::from(&engine),
            census: Some(census.to_string()),
        })
        .unwrap();
        let (got, exp) = (normal(&s.report, &want), normal(&want, &s.report));
        if got != exp {
            let line = got
                .lines()
                .zip(exp.lines())
                .enumerate()
                .find(|(_, (a, b))| a != b);
            panic!("{name}: survey.md differs at {line:?}");
        }
        // the provisional profile: the script's write_profile output, when the driver wrote it
        let py = sd.path().join("profile").join(format!("{app:016X}"));
        if py.join("game.tsv").is_file() {
            let pdir = tmp.join(&name).join(format!("{app:016X}"));
            // the script wrote the path as it was given: the target of the link, not its resolved form
            let base = std::fs::read_link(d.path()).unwrap_or_else(|_| d.path());
            let survey_path = base
                .join(sd.file_name())
                .join("survey.md")
                .to_string_lossy()
                .into_owned();
            assert!(write_profile(&pdir, app, &s, &survey_path).unwrap());
            for f in std::fs::read_dir(&py).unwrap().flatten() {
                let (a, b) = (
                    std::fs::read(f.path()).unwrap(),
                    std::fs::read(pdir.join(f.file_name())).unwrap(),
                );
                assert!(a == b, "{name}: {}", f.file_name().to_string_lossy());
            }
        }
        n += 1;
    }
    let _ = std::fs::remove_dir_all(&tmp);
    eprintln!("{n} surveys identical");
    assert!(n > 0);
}

/// `set_keys` and `refine_after_rewrite` against the script (`NXB_ORACLE_SK`: `<i>/{case.txt,input.tsv,expected.tsv}`
/// from the driver, `NXB_ORACLE_MU`: the patched module folders of the `mem_used` test).
#[test]
fn set_keys_and_refine_equal_the_reference_script() {
    let (Some(sk), Some(mu)) = (
        std::env::var_os("NXB_ORACLE_SK"),
        std::env::var_os("NXB_ORACLE_MU"),
    ) else {
        return;
    };
    let (sk, mu) = (PathBuf::from(sk), PathBuf::from(mu));
    let (mut n, mut changed) = (0, 0);
    for i in 0..60 {
        let d = sk.join(i.to_string());
        let case = std::fs::read_to_string(d.join("case.txt")).unwrap();
        let input = std::fs::read_to_string(d.join("input.tsv")).unwrap();
        let want = std::fs::read_to_string(d.join("expected.tsv")).unwrap();
        let mut lines = case.lines();
        let head: Vec<&str> = lines.next().unwrap().split('\t').collect();
        let got = if head[0] == "set" {
            let ch: Vec<(String, Option<(String, String)>)> = lines
                .map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    (
                        f[0].to_string(),
                        (f[1] != "None").then(|| (f[1].to_string(), f[2].to_string())),
                    )
                })
                .collect();
            set_keys(&input, &ch, head[1])
        } else {
            let ex = d.join("ex/ExeFS");
            let patched = mu.join(head[1]);
            refine_after_rewrite(&input, has_va(&ex).unwrap(), &patched)
                .unwrap()
                .unwrap_or_else(|| input.clone())
        };
        assert_eq!(got, want, "case {i}");
        changed += usize::from(input != want);
        n += 1;
    }
    eprintln!("{n} cases, {changed} change the profile");
}

#[test]
fn heap_and_region_results_stamp_the_profile() {
    let text = "title_id\t01\nmem_reserve\t0x140000000\nmem_alias\t0x118000000\nmem_stack\t0x10000000\nsys_resource\t0x1800000\napp_pool\t0xCD500000\n";
    let h = heap_check_result(
        text,
        "[mem] x\nSetHeapSize OOM: req 0x86000000\n",
        None,
        "2026-11-06",
    )
    .unwrap();
    assert_eq!(h.how, "the heap request did not fit");
    assert!(h.text.contains("\n# ---- Heap check on the Mac ----\n# the game asked for a 2.09 GB heap: a 2.125 GB heap region\nmem_alias\t0x98000000\n"));
    assert!(
        h.text
            .ends_with("\n# heap check: 2026-11-06, the heap request did not fit; set mem_alias\n")
    );
    assert!(heap_checked(&h.text));
    let h = heap_check_result(text, "", Some(1), "d").unwrap();
    assert_eq!(h.how, "the host stopped before nnMain (exit 1)");
    assert_eq!(
        h.text,
        format!("{text}\n# heap check: d, the host stopped before nnMain (exit 1)\n")
    );
    // no region table: the stamp says so, and a repeated check replaces it
    let r = region_check_result(text, "", "", &[], None, 15, "abc123", "d").unwrap();
    assert!(r.text.ends_with(
        "\n\n# address regions: d, no region table (the sdk has no region gates) (rewrite abc123)\n"
    ));
    assert!(region_checked(&r.text, "abc123") && !region_checked(&r.text, "other"));
    let r2 = region_check_result(&r.text, "", "", &[], Some(3), 15, "abc123", "e").unwrap();
    assert_eq!(r2.text.matches("# address regions: ").count(), 1);
    assert!(r2.text.contains("the host stopped before nnMain (exit 3)"));
}

#[test]
fn ipa_names() {
    assert_eq!(
        ipa_name("The Legend of Zelda: Link's Awakening", 1),
        "TheLegendOfZeldaLinksAwakening.ipa"
    );
    assert_eq!(ipa_name("NBA 2K", 1), "Nba2k.ipa");
    assert_eq!(
        ipa_name("\u{30b2}\u{30fc}\u{30e0}", 0xab),
        "00000000000000AB.ipa"
    );
}
