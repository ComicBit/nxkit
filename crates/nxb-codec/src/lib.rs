//! Decompressors and compressors used by extract, split, rewrite and the IPA writer.
//!
//! All decoders are total: bounded output, no panic on malformed input.
//!
//! Status: skeleton. The modules below are placeholders that fix the crate's shape, see
//! `docs/workspace-layout.md` for the planned public surface.

/// LZ4 block decoding (NSO segments).
pub mod lz4 {}

/// Yaz0 decoding.
pub mod yaz0 {}

/// LZSS variants.
pub mod lzss {}

/// CRILAYLA decoding.
pub mod crilayla {}

/// LZX decoding.
pub mod lzx {}

/// zstd decoding (the one reviewed external dependency, open question O14).
pub mod zstd {}

/// zlib inflate and deflate through the system library.
pub mod zlib {}

/// ZBIC, Nintendo's zstd variant (NSO flag bit 7).
pub mod zbic;
