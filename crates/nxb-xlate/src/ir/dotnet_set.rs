//! `HashSet<T>` enumeration order of .NET (no hashing needed): slots in insertion order, freed slots reused last-freed-first.

/// An insertion-ordered set that reproduces the enumeration order of `System.Collections.Generic.HashSet<T>`
/// (entries array plus a LIFO free list). Lookups are linear: the sets in the IR are small.
#[derive(Clone, Debug)]
pub struct DotnetSet<T: PartialEq + Copy> {
    entries: Vec<Option<T>>,
    free: Vec<usize>,
}

impl<T: PartialEq + Copy> Default for DotnetSet<T> {
    fn default() -> Self {
        DotnetSet {
            entries: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T: PartialEq + Copy> DotnetSet<T> {
    pub fn contains(&self, v: T) -> bool {
        self.entries.contains(&Some(v))
    }

    /// `Add`: false when already present.
    pub fn add(&mut self, v: T) -> bool {
        if self.contains(v) {
            return false;
        }
        match self.free.pop() {
            Some(i) => self.entries[i] = Some(v),
            None => self.entries.push(Some(v)),
        }
        true
    }

    /// `Remove`: false when absent.
    pub fn remove(&mut self, v: T) -> bool {
        match self.entries.iter().position(|e| *e == Some(v)) {
            Some(i) => {
                self.entries[i] = None;
                self.free.push(i);
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Enumeration in .NET order.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.entries.iter().filter_map(|e| *e)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.free.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_with_removals() {
        let mut s = DotnetSet::default();
        for i in 0..4 {
            s.add(i);
        }
        s.remove(1);
        s.remove(2);
        s.add(9); // reuses slot 2 (freed last)
        s.add(8); // then slot 1
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![0, 8, 9, 3]);
        assert_eq!(s.len(), 4);
    }
}
