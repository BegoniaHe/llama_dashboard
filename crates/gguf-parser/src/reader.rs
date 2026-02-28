//! GGUF file reader — quick-scan mode for fast metadata extraction.

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufReader, Read, Seek};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::types::*;

/// Maximum bytes to read in quick-scan mode.
/// For models with large tokenizer arrays (e.g. 151k tokens), 256 KiB is
/// insufficient.  We use 8 MiB which covers virtually all metadata while
/// still being fast (< 10 ms on modern hardware with OS page cache).
const QUICK_SCAN_LIMIT: u64 = 8 * 1024 * 1024;

//  Public result types

/// Outcome of a quick scan on a single `.gguf` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickScanResult {
    pub file_path: PathBuf,
    pub file_size: u64,
    pub header: GGUFHeader,
    pub architecture: Option<String>,
    pub name: Option<String>,
    pub file_type: Option<u32>,
    pub file_type_name: Option<String>,
    pub context_length: Option<u32>,
    pub embedding_length: Option<u32>,
    pub chat_template: Option<String>,
    /// All metadata KVs that fit within the scan window.
    pub metadata: Vec<GGUFMetadataKV>,
}

/// An entry in the model catalogue produced by [`scan_directory`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub file_size: u64,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub context_length: Option<u32>,
    pub is_split: bool,
    pub split_parts: Vec<PathBuf>,
    pub mmproj_path: Option<PathBuf>,
}

//  Quick scan

/// Read just enough of `path` to extract model metadata.
///
/// Targets < 10 ms per file on modern hardware.
pub fn quick_scan(path: &Path) -> Result<QuickScanResult, GGUFError> {
    let file = fs::File::open(path)?;
    let file_size = file.metadata()?.len();
    let limit = file_size.min(QUICK_SCAN_LIMIT);

    let mut reader = BufReader::new(file);

    //  Magic
    let magic = read_u32(&mut reader)?;
    if magic != GGUF_MAGIC {
        return Err(GGUFError::InvalidMagic(magic));
    }

    //  Version
    let version = read_u32(&mut reader)?;
    if version > GGUF_VERSION_MAX {
        return Err(GGUFError::UnsupportedVersion(version));
    }

    //  Counts
    let tensor_count = read_u64(&mut reader)?;
    let metadata_kv_count = read_u64(&mut reader)?;

    let header = GGUFHeader {
        version,
        tensor_count,
        metadata_kv_count,
    };

    //  Read metadata KVs (within the scan window)
    let mut metadata = Vec::new();
    for _ in 0..metadata_kv_count {
        let pos = reader.stream_position()?;
        if pos >= limit {
            break; // past our quick-scan window
        }
        match read_kv(&mut reader) {
            Ok(kv) => metadata.push(kv),
            Err(GGUFError::Io(ref e)) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
    }

    //  Extract well-known keys
    let kv_map: HashMap<&str, &GGUFValue> = metadata
        .iter()
        .map(|kv| (kv.key.as_str(), &kv.value))
        .collect();

    let architecture = kv_map
        .get("general.architecture")
        .and_then(|v| v.as_str())
        .map(String::from);

    let name = kv_map
        .get("general.name")
        .and_then(|v| v.as_str())
        .map(String::from);

    let file_type = kv_map.get("general.file_type").and_then(|v| v.as_u32());

    let ft_name = file_type.map(file_type_name).map(String::from);

    let arch = architecture.as_deref().unwrap_or("llama");
    let ctx_key = format!("{arch}.context_length");
    let context_length = kv_map.get(ctx_key.as_str()).and_then(|v| v.as_u32());

    let embd_key = format!("{arch}.embedding_length");
    let embedding_length = kv_map.get(embd_key.as_str()).and_then(|v| v.as_u32());

    let chat_template = kv_map
        .get("tokenizer.chat_template")
        .and_then(|v| v.as_str())
        .map(String::from);

    debug!(path = %path.display(), architecture = ?architecture, name = ?name, "quick scan complete");

    Ok(QuickScanResult {
        file_path: path.to_path_buf(),
        file_size,
        header,
        architecture,
        name,
        file_type,
        file_type_name: ft_name,
        context_length,
        embedding_length,
        chat_template,
        metadata,
    })
}

//  Directory scan

/// Recursively discover GGUF models in `dir`.
pub fn scan_directory(dir: &Path) -> Result<Vec<ModelEntry>, GGUFError> {
    let mut gguf_files: Vec<PathBuf> = Vec::new();
    walk_dir(dir, &mut gguf_files)?;
    gguf_files.sort();

    let mut entries: Vec<ModelEntry> = Vec::new();
    let mut seen_bases: HashMap<String, usize> = HashMap::new();

    for path in &gguf_files {
        let fname = path.file_name().unwrap_or_default().to_string_lossy();

        // Skip mmproj companion files (handled below).
        if fname.contains("-mmproj-") || fname.contains("_mmproj_") {
            continue;
        }

        // Detect split files: `name-00001-of-00003.gguf`
        if let Some(base) = detect_split_base(&fname) {
            if let Some(&idx) = seen_bases.get(&base) {
                entries[idx].split_parts.push(path.clone());
                entries[idx].is_split = true;
                continue;
            }
            // First part of a split set — create entry.
            seen_bases.insert(base.clone(), entries.len());
        }

        let scan = quick_scan(path).ok();
        let name = scan
            .as_ref()
            .and_then(|s| s.name.clone())
            .unwrap_or_else(|| fname.trim_end_matches(".gguf").to_string());

        let id = generate_model_id(path);

        entries.push(ModelEntry {
            id,
            name,
            path: path.clone(),
            file_size: scan.as_ref().map_or(0, |s| s.file_size),
            architecture: scan.as_ref().and_then(|s| s.architecture.clone()),
            quantization: scan.as_ref().and_then(|s| s.file_type_name.clone()),
            context_length: scan.as_ref().and_then(|s| s.context_length),
            is_split: false,
            split_parts: vec![path.clone()],
            mmproj_path: None,
        });
    }

    // Associate mmproj files with their parent model.
    for path in &gguf_files {
        let fname = path.file_name().unwrap_or_default().to_string_lossy();
        if !fname.contains("-mmproj-") && !fname.contains("_mmproj_") {
            continue;
        }
        // Try to match by directory (sibling) and name prefix.
        let parent = path.parent();
        for entry in &mut entries {
            if entry.path.parent() == parent && entry.mmproj_path.is_none() {
                entry.mmproj_path = Some(path.clone());
                break;
            }
        }
    }

    Ok(entries)
}

//  Internal helpers

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), GGUFError> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
            out.push(path);
        }
    }
    Ok(())
}

fn detect_split_base(filename: &str) -> Option<String> {
    // Pattern: `<base>-NNNNN-of-NNNNN.gguf`
    let name = filename.strip_suffix(".gguf")?;
    let parts: Vec<&str> = name.rsplitn(4, '-').collect();
    if parts.len() >= 4
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1] == "of"
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        Some(
            parts[3..]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>()
                .join("-"),
        )
    } else {
        None
    }
}

fn generate_model_id(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
        .replace(' ', "-")
}

//  Binary reading primitives

fn read_u32(r: &mut impl Read) -> Result<u32, GGUFError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)
        .map_err(|_| GGUFError::TruncatedHeader)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(r: &mut impl Read) -> Result<u64, GGUFError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)
        .map_err(|_| GGUFError::TruncatedHeader)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i8(r: &mut impl Read) -> Result<i8, GGUFError> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0] as i8)
}

fn read_u8(r: &mut impl Read) -> Result<u8, GGUFError> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_i16(r: &mut impl Read) -> Result<i16, GGUFError> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_u16(r: &mut impl Read) -> Result<u16, GGUFError> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i32(r: &mut impl Read) -> Result<i32, GGUFError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_i64(r: &mut impl Read) -> Result<i64, GGUFError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32(r: &mut impl Read) -> Result<f32, GGUFError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64(r: &mut impl Read) -> Result<f64, GGUFError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_string(r: &mut impl Read) -> Result<String, GGUFError> {
    let len = read_u64(r)? as usize;
    if len > 1_000_000 {
        return Err(GGUFError::Other(format!("string length {len} too large")));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn read_bool(r: &mut impl Read) -> Result<bool, GGUFError> {
    let v = read_u8(r)?;
    Ok(v != 0)
}

fn read_value(r: &mut impl Read, vtype: GGUFValueType) -> Result<GGUFValue, GGUFError> {
    match vtype {
        GGUFValueType::Uint8 => Ok(GGUFValue::Uint8(read_u8(r)?)),
        GGUFValueType::Int8 => Ok(GGUFValue::Int8(read_i8(r)?)),
        GGUFValueType::Uint16 => Ok(GGUFValue::Uint16(read_u16(r)?)),
        GGUFValueType::Int16 => Ok(GGUFValue::Int16(read_i16(r)?)),
        GGUFValueType::Uint32 => Ok(GGUFValue::Uint32(read_u32(r)?)),
        GGUFValueType::Int32 => Ok(GGUFValue::Int32(read_i32(r)?)),
        GGUFValueType::Float32 => Ok(GGUFValue::Float32(read_f32(r)?)),
        GGUFValueType::Bool => Ok(GGUFValue::Bool(read_bool(r)?)),
        GGUFValueType::String => Ok(GGUFValue::String(read_string(r)?)),
        GGUFValueType::Array => {
            let elem_type = GGUFValueType::try_from(read_u32(r)?)?;
            let count = read_u64(r)? as usize;
            if count > 10_000_000 {
                return Err(GGUFError::Other(format!("array length {count} too large")));
            }
            let mut arr = Vec::with_capacity(count.min(1024));
            for _ in 0..count {
                arr.push(read_value(r, elem_type)?);
            }
            Ok(GGUFValue::Array(arr))
        }
        GGUFValueType::Uint64 => Ok(GGUFValue::Uint64(read_u64(r)?)),
        GGUFValueType::Int64 => Ok(GGUFValue::Int64(read_i64(r)?)),
        GGUFValueType::Float64 => Ok(GGUFValue::Float64(read_f64(r)?)),
    }
}

fn read_kv(r: &mut impl Read) -> Result<GGUFMetadataKV, GGUFError> {
    let key = read_string(r)?;
    let vtype_raw = read_u32(r)?;
    let vtype = GGUFValueType::try_from(vtype_raw)?;
    let value = read_value(r, vtype)?;
    Ok(GGUFMetadataKV {
        key,
        value_type: vtype,
        value,
    })
}
