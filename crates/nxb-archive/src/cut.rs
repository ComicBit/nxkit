//! Batches cut by content. Bases are grouped by their vertex function (its library's bytes and entry name), the
//! groups ordered by their hash and cut into hash ranges ([`cut`]). Inserting or removing one base changes the batch
//! of its range only, so the store keeps every other batch.
//!
//! Grouping by vertex function matters for cost: `applegpu-nt` compiles each function once per run, so a batch of
//! bases that share vertex functions costs less (the reference title, 1000 bases: 291 vertex functions in pair order, 639 when
//! drawn at random, 10 % more CPU at random).

/// One base: its content hash, its vertex function's hash and its index in the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Base {
    /// SHA-256 of what identifies the base (libraries' bytes, entries, descriptor).
    pub hash: [u8; 32],
    /// SHA-256 of its vertex function (library bytes, entry).
    pub group: [u8; 32],
    /// Index in the script's pipeline list.
    pub index: usize,
}

fn u64_of(h: &[u8; 32]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&h[..8]);
    u64::from_be_bytes(a)
}

/// Cuts `bases` into batches: the vertex-function hash space is split into a power of two of equal ranges (about
/// `bases / target` of them), one batch per non-empty range, a batch over `2 * target` split further. An insertion
/// changes its range's batch only, until the base count crosses a power of two (then every batch changes once).
pub fn cut(mut bases: Vec<Base>, target: usize) -> Vec<Vec<Base>> {
    let target = target.max(1);
    bases.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then(a.hash.cmp(&b.hash))
            .then(a.index.cmp(&b.index))
    });
    let ranges = (bases.len() / target).max(1).next_power_of_two() as u64;
    let shift = 64 - ranges.trailing_zeros();
    let range_of = |b: &Base| {
        if ranges == 1 {
            0
        } else {
            u64_of(&b.group) >> shift
        }
    };
    let mut out: Vec<Vec<Base>> = Vec::new();
    let mut cur: Vec<Base> = Vec::new();
    for b in bases {
        let boundary = cur.last().is_some_and(|l| range_of(l) != range_of(&b));
        if !cur.is_empty() && (boundary || cur.len() >= 2 * target) {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(b);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bases(n: usize, salt: u8) -> Vec<Base> {
        (0..n)
            .map(|i| {
                let mut v = (i as u64).to_le_bytes().to_vec();
                v.push(salt);
                let g = nxb_model::sha256(&((i / 4) as u64).to_le_bytes()).0;
                Base {
                    hash: nxb_model::sha256(&v).0,
                    group: g,
                    index: i,
                }
            })
            .collect()
    }

    #[test]
    fn insertion_touches_at_most_three_batches() {
        let a = bases(20_000, 0);
        let before = cut(a.clone(), 1000);
        assert!(before.iter().all(|b| b.len() <= 2000));
        assert!(
            before.len() >= 16 && before.len() <= 40,
            "{} batches",
            before.len()
        );
        let total: usize = before.iter().map(Vec::len).sum();
        assert_eq!(total, 20_000);
        let mut b = a;
        b.extend(bases(1, 9).into_iter().map(|x| Base { index: 99_999, ..x }));
        let after = cut(b, 1000);
        let key = |v: &Vec<Base>| v.iter().map(|x| x.hash).collect::<Vec<_>>();
        let old: std::collections::BTreeSet<_> = before.iter().map(key).collect();
        let changed = after.iter().filter(|v| !old.contains(&key(v))).count();
        assert!(changed <= 3, "{changed} batches changed");
    }

    #[test]
    fn groups_stay_together() {
        let batches = cut(bases(8_000, 1), 500);
        let mut seen = std::collections::HashMap::new();
        for (i, b) in batches.iter().enumerate() {
            for x in b {
                seen.entry(x.group)
                    .or_insert_with(std::collections::BTreeSet::new)
                    .insert(i);
            }
        }
        let split = seen.values().filter(|s| s.len() > 1).count();
        assert!(
            split * 20 < seen.len(),
            "{split} of {} groups split",
            seen.len()
        );
    }
}
