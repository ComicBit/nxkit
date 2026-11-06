//! Runs every case of `cases.msl`. `tools/check_cases.py` checks the same file against the
//! Python prototype, so each expectation here is the prototype's verdict.

use std::collections::BTreeMap;
use std::rc::Rc;

use msl_parse::{check_with, prelude, template_hashes, Outcome};

struct Case {
    name: String,
    expect: String,
    prelude: Option<String>,
    src: String,
}

fn load() -> Vec<Case> {
    let text = include_str!("cases.msl");
    let mut cases: Vec<Case> = Vec::new();
    let mut header = false;
    let mut lines: Vec<&str> = Vec::new();
    let flush = |cases: &mut Vec<Case>, lines: &mut Vec<&str>| {
        if let Some(c) = cases.last_mut() {
            c.src = lines.join("\n");
        }
        lines.clear();
    };
    for line in text.split('\n') {
        if let Some(name) = line.strip_prefix("=== ") {
            flush(&mut cases, &mut lines);
            cases.push(Case {
                name: name.trim().into(),
                expect: String::new(),
                prelude: None,
                src: String::new(),
            });
            header = true;
            continue;
        }
        if cases.is_empty() {
            continue;
        }
        if header {
            if let Some(e) = line.strip_prefix("expect: ") {
                cases.last_mut().unwrap().expect = e.trim().into();
                continue;
            }
            if let Some(p) = line.strip_prefix("prelude: ") {
                cases.last_mut().unwrap().prelude = Some(p.trim().into());
                continue;
            }
        }
        header = false;
        lines.push(line);
    }
    flush(&mut cases, &mut lines);
    cases
}

/// The verdict in the case file's notation, with reason keys sorted.
fn verdict(out: &Outcome) -> String {
    match out {
        Outcome::Checked(c) if c.reasons.is_empty() => "accept".into(),
        Outcome::Checked(c) => {
            let mut keys: Vec<String> = Vec::new();
            for (k, n) in c.reasons.iter() {
                for _ in 0..n {
                    keys.push(k.to_string());
                }
            }
            keys.sort();
            format!("reject {}", keys.join(" | "))
        }
        Outcome::Recursion => "recursion".into(),
        Outcome::LexError(_) => "lex-error".into(),
        Outcome::Crash(e) => format!("crash:{}", e.name()),
    }
}

fn canon(expect: &str) -> String {
    match expect.strip_prefix("reject ") {
        Some(rest) => {
            let mut keys: Vec<&str> = rest.split(" | ").collect();
            keys.sort();
            format!("reject {}", keys.join(" | "))
        }
        None => expect.to_string(),
    }
}

fn run_case(c: &Case) -> Outcome {
    let table = c.prelude.as_ref().map(|name| {
        let mut t = prelude::Table::builtin();
        for h in template_hashes(&c.src).expect("case lexes") {
            t = t.with(&h, name);
        }
        Rc::new(t)
    });
    check_with(&c.src, table)
}

#[test]
fn all_cases() {
    let cases = load();
    assert!(cases.len() > 100, "case file not parsed");
    let mut failures = Vec::new();
    for c in &cases {
        let got = verdict(&run_case(c));
        if got != canon(&c.expect) {
            failures.push(format!(
                "{}: expected {:?}, got {:?}",
                c.name, c.expect, got
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn case_names_are_unique() {
    let mut seen = BTreeMap::new();
    for c in load() {
        *seen.entry(c.name).or_insert(0) += 1;
    }
    let dups: Vec<_> = seen.into_iter().filter(|(_, n)| *n > 1).collect();
    assert!(dups.is_empty(), "duplicate case names: {dups:?}");
}

/// Every reason key the parser can produce, with a case that produces it.
#[test]
fn every_reject_reason_has_a_case() {
    let expected_keys = [
        "pp:",
        "template:",
        "struct-forward",
        "field-attr:",
        "global-constant-form",
        "fn-attr:",
        "gnu-attr:",
        "param-attr:",
        "function_constant-unknown",
        "fn-prototype",
        "array-dim-nonliteral",
        "texture-elem:",
        "texture-kind:",
        "spvUnsafeArray-without-prelude",
        "type:",
        "thread-reference",
        "addrspace-value:",
        "switch-non-int",
        "case-label:",
        "return-missing-value",
        "brace-init-nonempty",
        "decl-ctor-syntax",
        "condition-not-scalar",
        "implicit-conversion",
        "comma-operator",
        "assign-to-rvalue",
        "ternary-types",
        "matrix-op",
        "binary-nonnumeric",
        "logical-vector",
        "vector-width-mismatch",
        "vector-base-mismatch",
        "bitop-on-float",
        "deref-nonpointer",
        "address-of-rvalue",
        "prefix-incdec",
        "not-vector",
        "unary-nonnumeric",
        "bitnot-float",
        "arrow-nonpointer",
        "index-non-int",
        "matrix-index",
        "index-on:",
        "postfix-incdec",
        "unknown-member",
        "swizzle-form",
        "swizzle-out-of-range",
        "swizzle-on-scalar",
        "member-on:",
        "c-cast",
        "syntax",
        "qualified:",
        "packed-ctor",
        "cast-args",
        "as_type-size",
        "reinterpret_cast",
        "struct-init-too-many",
        "option-not-call",
        "function-pointer",
        "unknown-identifier",
        "matrix-ctor",
        "scalar-ctor-form",
        "ctor-arg-nonnumeric",
        "ctor-component-count",
        "call-entry-point",
        "overload-resolution",
        "dm_bs-form",
        "dm_bs-instantiation",
        "spvDepthCast-form",
        "spvFAdd-form",
        "option-args",
        "builtin-arity:",
        "builtin-nonnumeric:",
        "builtin-width-mix:",
        "builtin-arg-types-mixed:",
        "builtin-int-to-float-fn:",
        "builtin-float-to-int-fn:",
        "builtin-form:",
        "select-form",
        "select-cond",
        "select-type-mix",
        "all-any-form",
        "unpack-arg",
        "extract_bits-form",
        "insert_bits-form",
        "mulhi-form",
        "simd_shuffle-form",
        "simd-vote-form",
        "call:",
        "method-on:",
        "sample-no-sampler",
        "sample-coord",
        "sample-array-index",
        "sample_compare-nondepth",
        "sample_compare-ref",
        "gather-option",
        "sample-arg",
        "read-args",
        "lod-form",
        "method:",
    ];
    let cases = load();
    let mut missing = Vec::new();
    for key in expected_keys {
        let covered = cases.iter().any(|c| {
            c.expect.strip_prefix("reject ").is_some_and(|r| {
                r.split(" | ").any(|k| {
                    if key.ends_with(':') {
                        k.starts_with(key)
                    } else {
                        k == key
                    }
                })
            })
        });
        if !covered {
            missing.push(key);
        }
    }
    assert!(missing.is_empty(), "reasons without a case: {missing:?}");
}
