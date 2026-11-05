//! Parallel I/O for the stages (layer L2).
//!
//! Present: [`hash_files`] and [`hash_tree`]: hash many files in parallel, each file read once, largest first, results
//! in a stable order. They are for files nobody hashed at production time (inputs from outside, a tree after an unclean
//! end), a producer's digest from the store manifest always wins (design-pipeline-rest 8, "hash once").
//!
//! Planned: `ParallelReader` (block reads per device), `OrderedWriter` (planned-offset writes with in-order hashing),
//! `clone_tree`, a digest index keyed by stat fingerprint.
//! The `_dual` variants return SHA-1 beside SHA-256 ([`nxb_model::DualDigest`]) for `CodeResources` seals.

mod hash;

pub use hash::{
    FileHash, TreeEntry, TreeHash, TreeSymlink, hash_file, hash_files, hash_files_dual, hash_tree,
    hash_tree_dual,
};
