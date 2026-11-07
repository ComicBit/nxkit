//! The one list of families and title modules.
//!
//! Each module is behind its own cargo feature (`agl`, `oot`), so a build can leave any of them out. This is the
//! only crate that depends on family and title crates. The core depends on this crate and on `nxtitle`.

use nxtitle::{Catalog, Family, Title};

/// The catalog of every module this build holds.
pub fn catalog() -> Result<Catalog, String> {
    let families: Vec<&'static dyn Family> = vec![
        #[cfg(feature = "agl")]
        &nxfamily_agl::AGL,
    ];
    let titles: Vec<&'static dyn Title> = vec![
        #[cfg(feature = "oot")]
        &nxgame_oot::OOT,
    ];
    Catalog::new(families, titles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_builds() {
        let c = catalog().unwrap();
        #[cfg(all(feature = "agl", feature = "oot"))]
        assert!(c.missing_families().is_empty());
        #[cfg(all(feature = "oot", not(feature = "agl")))]
        assert_eq!(c.missing_families().len(), 1);
        #[cfg(not(any(feature = "agl", feature = "oot")))]
        assert!(c.titles().is_empty() && c.families().is_empty());
    }

    #[cfg(all(feature = "agl", feature = "oot"))]
    #[test]
    fn oot_resolves_to_agl() {
        use nxtitle::{FamilyFrom, TitleFacts, TitleId};
        let c = catalog().unwrap();
        let r = c.resolve(&TitleFacts::new(TitleId(0x0400_4860_24DE_C000)));
        assert_eq!(
            r.title.map(|t| t.name()),
            Some("The Legend of Zelda: Ocarina of Time")
        );
        assert_eq!(r.family.map(|f| f.id()), Some("agl"));
        assert_eq!(r.family_from, FamilyFrom::Declared);
        assert_eq!(r.needs().len(), 6);
    }
}
