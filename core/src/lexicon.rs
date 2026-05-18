//! Memory-mapped lexicon for zero-copy dictionary lookup.
//!
//! FST maps phonetic keys to indices, flat payload stores entries.
//! Only pages in data on demand, keeping RSS minimal.

use memmap2::Mmap;
use std::path::Path;

/// File format for lexicon.dat:
///
/// ```text
/// Header (16 bytes):
///   magic: [u8; 4] = b"LXPD"
///   version: u32 = 1
///   num_keys: u32
///   _reserved: u32
///
/// Key offsets: [u32; num_keys + 1] — byte offset into entries section
/// Entries section: for each key, packed entries:
///   Each entry: u16 str_len + str_bytes + u32 freq
/// ```
///
/// lexicon.fst: FST mapping pinyin_key → u64 index into key_offsets

const LXPD_MAGIC: &[u8; 4] = b"LXPD";
const LXPD_VERSION: u32 = 1;
const LXPD_HEADER_SIZE: usize = 16;

/// Memory-mapped lexicon. FST for key lookup, flat payload for entries.
pub struct Lexicon {
    fst_map: fst::Map<Mmap>,
    payload_mmap: Mmap,
    num_keys: u32,
}

impl Lexicon {
    /// Load from lexicon.fst + lexicon.dat
    pub fn load(fst_path: &Path, dat_path: &Path) -> Result<Self, String> {
        let fst_file =
            std::fs::File::open(fst_path).map_err(|e| format!("open {:?}: {}", fst_path, e))?;
        let dat_file =
            std::fs::File::open(dat_path).map_err(|e| format!("open {:?}: {}", dat_path, e))?;

        let fst_mmap = unsafe { Mmap::map(&fst_file) }.map_err(|e| format!("mmap fst: {}", e))?;
        let payload_mmap =
            unsafe { Mmap::map(&dat_file) }.map_err(|e| format!("mmap dat: {}", e))?;

        if payload_mmap.len() < LXPD_HEADER_SIZE {
            return Err("lexicon.dat too small".into());
        }
        if &payload_mmap[0..4] != LXPD_MAGIC {
            return Err("lexicon.dat: bad magic".into());
        }
        let version = u32::from_le_bytes(payload_mmap[4..8].try_into().unwrap());
        if version != LXPD_VERSION {
            return Err(format!("lexicon.dat: unsupported version {}", version));
        }
        let num_keys = u32::from_le_bytes(payload_mmap[8..12].try_into().unwrap());

        let fst_map = fst::Map::new(fst_mmap).map_err(|e| format!("fst parse: {}", e))?;

        Ok(Self {
            fst_map,
            payload_mmap,
            num_keys,
        })
    }

    #[inline]
    fn key_offsets_start(&self) -> usize {
        LXPD_HEADER_SIZE
    }

    #[inline]
    fn entries_start(&self) -> usize {
        LXPD_HEADER_SIZE + (self.num_keys as usize + 1) * 4
    }

    #[inline]
    fn get_key_offset(&self, idx: u32) -> u32 {
        let off = self.key_offsets_start() + (idx as usize) * 4;
        u32::from_le_bytes(self.payload_mmap[off..off + 4].try_into().unwrap())
    }

    fn read_entries(&self, idx: u32) -> Vec<(String, u32)> {
        let start = self.entries_start() + self.get_key_offset(idx) as usize;
        let end = self.entries_start() + self.get_key_offset(idx + 1) as usize;
        let data = &self.payload_mmap[start..end];

        let mut results = Vec::new();
        let mut pos = 0;
        while pos + 2 <= data.len() {
            let str_len = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            pos += 2;
            if pos + str_len + 4 > data.len() {
                break;
            }
            let s = std::str::from_utf8(&data[pos..pos + str_len]).unwrap_or("");
            pos += str_len;
            let freq = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            pos += 4;
            results.push((s.to_string(), freq));
        }
        results
    }

    /// Lookup candidates for a given phonetic key.
    pub fn lookup(&self, key: &str) -> Vec<String> {
        if let Some(idx) = self.fst_map.get(key) {
            self.read_entries(idx as u32)
                .into_iter()
                .map(|(s, _)| s)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Lookup with frequencies.
    pub fn lookup_with_freq(&self, key: &str) -> Vec<(String, u32)> {
        if let Some(idx) = self.fst_map.get(key) {
            self.read_entries(idx as u32)
        } else {
            Vec::new()
        }
    }

    /// Check if a key exists (FST only, no payload access).
    pub fn has_key(&self, key: &str) -> bool {
        self.fst_map.get(key).is_some()
    }
}
