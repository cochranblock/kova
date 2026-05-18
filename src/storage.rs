#![allow(non_camel_case_types)]
//! Storage layer. redb + bincode + zstd. Shared single-file DB via global Arc.

// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6, SuperNinja, Composer 1.5, Google Gemini Pro 3

use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use thiserror::Error;

/// Single table used by the t12 KV abstraction.
pub(crate) const KV_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("kv");

/// E0 = StorageError.
#[derive(Debug, Error)]
pub enum E0 {
    #[error("db error: {0}")]
    Db(String),
    #[error("bincode encode failed: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    #[error("bincode decode failed: {0}")]
    BincodeDecode(#[from] bincode::error::DecodeError),
    #[error("zstd compress failed: {0}")]
    ZstdCompress(std::io::Error),
    #[error("zstd decompress failed: {0}")]
    ZstdDecompress(std::io::Error),
}

// ── Global shared DB ─────────────────────────────────────────────────────────

static GLOBAL_DB: OnceLock<Option<Arc<Database>>> = OnceLock::new();

/// Lazily open the global redb Database at `db_path()`. All modules that
/// need the primary KV store should call this rather than opening their own
/// Database — redb requires exclusive file access.
pub fn global_db() -> Option<Arc<Database>> {
    GLOBAL_DB
        .get_or_init(|| {
            let path = crate::config::db_path();
            if let Some(parent) = path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("[storage] cannot create {}: {e}", parent.display());
            }
            Database::create(&path)
                .map(Arc::new)
                .map_err(|e| eprintln!("[storage] redb open failed: {e}"))
                .ok()
        })
        .clone()
}

// ── t12 ──────────────────────────────────────────────────────────────────────

enum t12Inner {
    Shared(Arc<Database>),
    Owned { db: Arc<Database>, _tmp: tempfile::TempDir },
}

/// t12 = Store. redb-backed KV with bincode+zstd payloads.
pub struct t12 {
    inner: t12Inner,
}

impl t12 {
    fn db(&self) -> &Database {
        match &self.inner {
            t12Inner::Shared(arc) => arc,
            t12Inner::Owned { db, .. } => db,
        }
    }

    /// f39 = open. Returns a handle backed by the global shared Database.
    pub fn f39() -> Result<Self, E0> {
        let db = global_db().ok_or_else(|| E0::Db("global DB unavailable".into()))?;
        Ok(Self { inner: t12Inner::Shared(db) })
    }

    /// Temporary isolated DB backed by a tempdir. For tests only.
    pub fn temporary() -> Result<Self, E0> {
        let tmp = tempfile::TempDir::new().map_err(|e| E0::Db(e.to_string()))?;
        let db = Database::create(tmp.path().join("kv.redb"))
            .map_err(|e| E0::Db(e.to_string()))?;
        Ok(Self { inner: t12Inner::Owned { db: Arc::new(db), _tmp: tmp } })
    }

    /// f40 = put_compressed. bincode-encode, zstd-compress, then store.
    pub fn f40<K: AsRef<[u8]>, V: Serialize>(&self, key: K, value: &V) -> Result<(), E0> {
        let encoded = bincode::serde::encode_to_vec(value, bincode::config::standard())?;
        let compressed = zstd::encode_all(encoded.as_slice(), 3).map_err(E0::ZstdCompress)?;
        let txn = self.db().begin_write().map_err(|e| E0::Db(e.to_string()))?;
        {
            let mut table = txn.open_table(KV_TABLE).map_err(|e| E0::Db(e.to_string()))?;
            table
                .insert(key.as_ref(), compressed.as_slice())
                .map_err(|e| E0::Db(e.to_string()))?;
        }
        txn.commit().map_err(|e| E0::Db(e.to_string()))?;
        Ok(())
    }

    /// f41 = get_compressed. Fetch, zstd-decompress, bincode-decode.
    pub fn f41<K: AsRef<[u8]>, V: for<'de> Deserialize<'de>>(
        &self,
        key: K,
    ) -> Result<Option<V>, E0> {
        let txn = self.db().begin_read().map_err(|e| E0::Db(e.to_string()))?;
        let table = match txn.open_table(KV_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None), // table not yet created
        };
        let Some(guard) = table.get(key.as_ref()).map_err(|e| E0::Db(e.to_string()))? else {
            return Ok(None);
        };
        let decompressed =
            zstd::decode_all(guard.value()).map_err(E0::ZstdDecompress)?;
        let (decoded, _) =
            bincode::serde::decode_from_slice(&decompressed, bincode::config::standard())?;
        Ok(Some(decoded))
    }

    /// f42 = put_raw. Store raw bytes without compression.
    pub fn f42<K: AsRef<[u8]>>(&self, key: K, value: &[u8]) -> Result<(), E0> {
        let txn = self.db().begin_write().map_err(|e| E0::Db(e.to_string()))?;
        {
            let mut table = txn.open_table(KV_TABLE).map_err(|e| E0::Db(e.to_string()))?;
            table
                .insert(key.as_ref(), value)
                .map_err(|e| E0::Db(e.to_string()))?;
        }
        txn.commit().map_err(|e| E0::Db(e.to_string()))?;
        Ok(())
    }

    /// f43 = get_raw. Returns owned bytes (was sled::IVec; now Vec<u8>).
    pub fn f43<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, E0> {
        let txn = self.db().begin_read().map_err(|e| E0::Db(e.to_string()))?;
        let table = match txn.open_table(KV_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        Ok(table
            .get(key.as_ref())
            .map_err(|e| E0::Db(e.to_string()))?
            .map(|g| g.value().to_vec()))
    }

    /// f44 = scan_prefix. Iterate all keys with the given prefix, yield (key, raw_bytes).
    #[allow(clippy::type_complexity)]
    pub fn f44(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, E0> {
        let txn = self.db().begin_read().map_err(|e| E0::Db(e.to_string()))?;
        let table = match txn.open_table(KV_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(vec![]),
        };
        let mut end = prefix.to_vec();
        // increment last byte to form exclusive upper bound; if overflow, use unbounded
        let range: Vec<(Vec<u8>, Vec<u8>)> = if let Some(last) = end.last_mut() {
            *last = last.wrapping_add(1);
            let lo: &[u8] = prefix;
            let hi: &[u8] = &end;
            table
                .range(lo..hi)
                .map_err(|e| E0::Db(e.to_string()))?
                .filter_map(|r| r.ok())
                .map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
                .collect()
        } else {
            vec![]
        };
        Ok(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestVal {
        x: i32,
        s: String,
    }

    /// f39+f40+f41=open,put_compressed,get_compressed (isolated temp DB)
    #[test]
    fn store_put_get_compressed_roundtrip() {
        let store = t12::temporary().unwrap();
        let v = TestVal { x: 42, s: "hello".into() };
        store.f40(b"key1", &v).unwrap();
        let got: Option<TestVal> = store.f41(b"key1").unwrap();
        assert_eq!(got, Some(v));
    }

    /// f41=get_compressed missing key
    #[test]
    fn store_get_missing_returns_none() {
        let store = t12::temporary().unwrap();
        let got: Option<TestVal> = store.f41(b"missing").unwrap();
        assert_eq!(got, None);
    }

    /// f42+f43=put_raw,get_raw
    #[test]
    fn store_put_raw_get_raw() {
        let store = t12::temporary().unwrap();
        store.f42(b"raw", b"bytes").unwrap();
        let got = store.f43(b"raw").unwrap();
        assert_eq!(got.as_deref(), Some(b"bytes" as &[u8]));
    }
}
