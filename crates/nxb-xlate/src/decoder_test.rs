//! Corpus test of the Maxwell decoder (needs the job accessor).
#![cfg(test)]

mod tests {
    use crate::accessor::Accessor;
    use crate::attr::Stage;
    use crate::maxwell::decoder::{decode, dump};

    #[test]
    fn corpus_decoded_dump() {
        let Some(entries) = crate::corpus::load() else {
            return;
        };
        let (mut n, mut same) = (0, 0);
        for e in &entries {
            let Some(want) = e.first_attempt("decoded") else {
                continue;
            };
            let mut acc = Accessor::new(&e.job);
            let (stage, start) = match e.job.stage.as_str() {
                "vs" => (Stage::Vertex, 0x50),
                "ps" => (Stage::Fragment, 0x50),
                _ => (Stage::Compute, 0),
            };
            n += 1;
            match decode(stage, &mut acc, &e.code, start) {
                Ok((p, _)) if dump(&p) == want => same += 1,
                Ok((p, _)) => {
                    if n - same < 3 {
                        let got = dump(&p);
                        let l = got.lines().zip(want.lines()).position(|(a, b)| a != b);
                        eprintln!(
                            "differs {}: line {:?}\n got {:?}\nwant {:?}",
                            e.job.key,
                            l,
                            l.and_then(|i| got.lines().nth(i)),
                            l.and_then(|i| want.lines().nth(i))
                        );
                    }
                }
                Err(err) => eprintln!("{}: {err}", e.job.key),
            }
        }
        eprintln!("decoder identical: {same} of {n}");
        assert_eq!(same, n);
    }
}
