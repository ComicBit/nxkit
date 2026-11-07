//! The game's own decoder (`oodle`, `oodle_supposed` of the reference): Oodle streams and LZ5 blocks decode with the
//! copy linked into main, in the Mac host. This crate stays pure: the stage installs a [`Decoder`] for the length of
//! a split (`nxb-extract`'s serve client), and without one every such stream is [`SplitError::Unsupported`].

use std::sync::{Arc, RwLock};

use crate::SplitError;

/// One answer of the decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The decoded bytes, exactly the length asked for.
    Bytes(Vec<u8>),
    /// The decoder refused the stream and goes on (it answered 0).
    Refused,
    /// The host is gone. The text is what it wrote, for the error.
    Gone(String),
}

/// The decoder: (stored bytes, raw length) to an [`Answer`].
pub trait Decoder: Send + Sync {
    /// Decodes one stream.
    fn ask(&self, comp: &[u8], n: usize) -> Answer;
    /// What the host wrote so far (the error text of a refused stream).
    fn said(&self) -> String;
}

static DECODER: RwLock<Option<Arc<dyn Decoder>>> = RwLock::new(None);

/// Installs (or with `None` removes) the decoder.
pub fn set_decoder(d: Option<Arc<dyn Decoder>>) {
    if let Ok(mut g) = DECODER.write() {
        *g = d;
    }
}

fn current() -> Option<Arc<dyn Decoder>> {
    DECODER.read().ok().and_then(|g| g.clone())
}

/// Whether a decoder is installed.
pub fn installed() -> bool {
    current().is_some()
}

fn last(s: &str, n: usize) -> &str {
    let mut i = s.len().saturating_sub(n);
    while !s.is_char_boundary(i) {
        i += 1;
    }
    &s[i..]
}

fn failed(comp: &[u8], n: usize, said: &str) -> SplitError {
    SplitError::Red(format!(
        "the game's decoder in the Mac host failed on a {} → {n} byte stream; ---- host output ----\n{}",
        comp.len(),
        last(said, 3000)
    ))
}

/// `oodle`: the decoded stream. `what` names the data when no decoder is installed.
pub fn decode(comp: &[u8], n: usize, what: &str) -> Result<Vec<u8>, SplitError> {
    let Some(d) = current() else {
        return Err(SplitError::Unsupported(format!(
            "{what}: Oodle- or LZ5-compressed game files need the game's own decoder in the Mac host (the build gives it when the profile's shader sources name Oodle packs or legacy Unreal paks)"
        )));
    };
    match d.ask(comp, n) {
        Answer::Bytes(b) => Ok(b),
        Answer::Refused => Err(failed(comp, n, &d.said())),
        Answer::Gone(s) => Err(failed(comp, n, &s)),
    }
}

/// `oodle_supposed`: a stream a blind scan only supposes. `None` when the decoder refuses it, or main has no
/// decoder to refuse it. A host gone for another reason is red. Without an installed decoder: `None`.
pub fn supposed(comp: &[u8], n: usize) -> Result<Option<Vec<u8>>, SplitError> {
    let Some(d) = current() else {
        return Ok(None);
    };
    match d.ask(comp, n) {
        Answer::Bytes(b) => Ok(Some(b)),
        Answer::Refused => Ok(None),
        Answer::Gone(s) if s.contains("no streaming decoder to find") => Ok(None),
        Answer::Gone(s) => Err(failed(comp, n, &s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Decoder for Fake {
        fn ask(&self, comp: &[u8], n: usize) -> Answer {
            match comp.first() {
                Some(0) => Answer::Refused,
                Some(1) => Answer::Gone("x no streaming decoder to find y".into()),
                Some(2) => Answer::Gone("crash".into()),
                _ => Answer::Bytes(vec![7; n]),
            }
        }
        fn said(&self) -> String {
            "said".into()
        }
    }

    #[test]
    fn decode_and_supposed() {
        set_decoder(None);
        assert!(matches!(
            decode(&[9], 3, "x"),
            Err(SplitError::Unsupported(_))
        ));
        assert_eq!(supposed(&[9], 3), Ok(None));
        set_decoder(Some(Arc::new(Fake)));
        assert_eq!(decode(&[9], 3, "x"), Ok(vec![7; 3]));
        assert!(matches!(decode(&[0], 3, "x"), Err(SplitError::Red(m)) if m.ends_with("said")));
        assert_eq!(supposed(&[0], 3), Ok(None));
        assert_eq!(supposed(&[1], 3), Ok(None));
        assert!(matches!(supposed(&[2], 3), Err(SplitError::Red(m)) if m.ends_with("crash")));
        set_decoder(None);
        assert!(!installed());
    }
}
