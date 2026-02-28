//! Pure-Rust GGUF file format parser.
//!
//! Provides fast metadata extraction from `.gguf` files **without**
//! depending on llama.cpp.  Two modes are supported:
//!
//! * **quick scan** — reads only the first ~128 KiB to extract model
//!   name, architecture, quantisation type, context length, etc.
//! * **directory scan** — recursively discovers all `.gguf` models in
//!   a directory tree, grouping split files and detecting mmproj
//!   companions.

pub mod reader;
pub mod types;

pub use reader::{ModelEntry, QuickScanResult, quick_scan, scan_directory};
pub use types::{GGUFHeader, GGUFMetadataKV, GGUFValue, GGUFValueType, file_type_name};
