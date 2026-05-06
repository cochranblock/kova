//! nanobyte — packed model file format. mmap-loadable, BLAKE3-signed.
//!
//! Layout:
//!   `[HEADER 64B] [MANIFEST] [WEIGHTS] [NSIG 36B]`
//!
//! - HEADER: magic "NANO", version, num_models, manifest offset/size, total weight bytes.
//! - MANIFEST: `num_models` entries of [`MANIFEST_ENTRY_SIZE`] bytes each.
//! - WEIGHTS: contiguous f32 blob, indexed via per-model offsets.
//! - NSIG trailer: b"NSIG" + 32-byte BLAKE3 of every byte before. See `docs/NANOSIGN.md`.
//!
//! Spec: `docs/KOVA_BLUEPRINT.md` §2.
// Unlicense — cochranblock.org
// Contributors: Mattbusel (XFactor), GotEmCoach, KOVA, Claude Opus 4.6

use std::fs;
use std::io::Write;
use std::path::Path;

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

/// File magic: ASCII `NANO`.
pub const MAGIC: [u8; 4] = *b"NANO";

/// Format version.
pub const VERSION: u32 = 1;

/// Header size in bytes.
pub const HEADER_SIZE: usize = 64;

/// Manifest entry size in bytes. 8-byte aligned.
pub const MANIFEST_ENTRY_SIZE: usize = 80;

/// NanoSign trailer: 4-byte `NSIG` magic + 32-byte BLAKE3.
pub const NSIG_SIZE: usize = 36;

/// NanoSign magic.
pub const NSIG_MAGIC: [u8; 4] = *b"NSIG";

/// Maximum model name length (zero-padded inside manifest entry).
pub const NAME_LEN: usize = 32;

/// Errors from nanobyte load/pack.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("file too small ({0} bytes)")]
    TooSmall(usize),
    #[error("bad magic: expected NANO, got {0:?}")]
    BadMagic([u8; 4]),
    #[error("unsupported version: {0}")]
    BadVersion(u32),
    #[error("missing NSIG trailer")]
    Unsigned,
    #[error("NSIG verification failed (file tampered or corrupted)")]
    BadSignature,
    #[error("manifest extends past file or has bad size")]
    BadManifest,
    #[error("model {0:?} not found")]
    NotFound(String),
    #[error("model name longer than 32 bytes: {0:?}")]
    NameTooLong(String),
    #[error("weights region misaligned for f32 (byte {0})")]
    Misaligned(u64),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One model's manifest entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub name: String,
    /// 1=subatomic, 2=molecular, 3=cellular.
    pub tier: u8,
    pub num_classes: u32,
    pub feature_dim: u32,
    /// Weights byte offset, relative to the start of the weights region.
    pub offset: u64,
    /// Weight byte count.
    pub size: u64,
    /// Routing weights offset (T2/T3 only). 0 if absent.
    pub routing_offset: u64,
    pub routing_size: u64,
}

/// One model to pack into a nanobyte file.
pub struct PackInput<'a> {
    pub name: &'a str,
    pub tier: u8,
    pub num_classes: u32,
    pub feature_dim: u32,
    pub weights: &'a [f32],
    pub routing: Option<&'a [f32]>,
}

/// Backing storage for a loaded nanobyte. Either a memory-mapped file or a
/// `'static` byte slice (e.g. from `include_bytes!`).
enum Storage {
    Mmap(Mmap),
    Static(&'static [u8]),
}

impl Storage {
    fn as_slice(&self) -> &[u8] {
        match self {
            Storage::Mmap(m) => &m[..],
            Storage::Static(s) => s,
        }
    }
}

/// Loaded nanobyte file. Holds its backing storage alive.
pub struct Nanobyte {
    storage: Storage,
    manifests: Vec<Manifest>,
    /// Absolute byte offset of the weights region within the file.
    weights_start: u64,
}

impl Nanobyte {
    /// Open and verify a nanobyte file via mmap. Validates magic, version, and NSIG.
    pub fn load(path: &Path) -> Result<Self> {
        let file = fs::File::open(path)?;
        // SAFETY: callers must not mutate the file while a Nanobyte holds it.
        let mmap = unsafe { Mmap::map(&file)? };
        Self::from_storage(Storage::Mmap(mmap))
    }

    /// Verify and parse a nanobyte from a `'static` byte slice (e.g. `include_bytes!`).
    /// Zero-copy: the slice is borrowed for the lifetime of the returned `Nanobyte`.
    pub fn from_bytes(bytes: &'static [u8]) -> Result<Self> {
        Self::from_storage(Storage::Static(bytes))
    }

    fn from_storage(storage: Storage) -> Result<Self> {
        let buf = storage.as_slice();
        let len = buf.len();
        if len < HEADER_SIZE + NSIG_SIZE {
            return Err(Error::TooSmall(len));
        }

        let payload_end = len - NSIG_SIZE;
        let trailer = &buf[payload_end..];
        if trailer[..4] != NSIG_MAGIC {
            return Err(Error::Unsigned);
        }
        let expected = &trailer[4..];
        let actual = blake3::hash(&buf[..payload_end]);
        if actual.as_bytes() != expected {
            return Err(Error::BadSignature);
        }

        let h = &buf[..HEADER_SIZE];
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&h[0..4]);
        if magic != MAGIC {
            return Err(Error::BadMagic(magic));
        }
        let version = u32::from_le_bytes(h[4..8].try_into().unwrap());
        if version != VERSION {
            return Err(Error::BadVersion(version));
        }
        let num_models = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
        let manifest_offset = u64::from_le_bytes(h[12..20].try_into().unwrap());
        let manifest_size = u64::from_le_bytes(h[20..28].try_into().unwrap());

        let manifest_end = manifest_offset
            .checked_add(manifest_size)
            .ok_or(Error::BadManifest)?;
        if (manifest_end as usize) > payload_end {
            return Err(Error::BadManifest);
        }
        if manifest_size as usize != num_models * MANIFEST_ENTRY_SIZE {
            return Err(Error::BadManifest);
        }

        let mut manifests = Vec::with_capacity(num_models);
        for i in 0..num_models {
            let start = manifest_offset as usize + i * MANIFEST_ENTRY_SIZE;
            let entry = &buf[start..start + MANIFEST_ENTRY_SIZE];
            manifests.push(decode_manifest(entry));
        }

        Ok(Self {
            storage,
            manifests,
            weights_start: manifest_end,
        })
    }

    pub fn manifests(&self) -> &[Manifest] {
        &self.manifests
    }

    fn find(&self, name: &str) -> Result<&Manifest> {
        self.manifests
            .iter()
            .find(|m| m.name == name)
            .ok_or_else(|| Error::NotFound(name.to_string()))
    }

    /// Return weights for a model as `&[f32]`.
    pub fn weights(&self, name: &str) -> Result<&[f32]> {
        let m = self.find(name)?;
        slice_f32(self.storage.as_slice(), self.weights_start + m.offset, m.size)
    }

    /// Return routing weights for a model (T2/T3 only). `None` if model has no routing block.
    pub fn routing(&self, name: &str) -> Result<Option<&[f32]>> {
        let m = self.find(name)?;
        if m.routing_size == 0 {
            return Ok(None);
        }
        Ok(Some(slice_f32(
            self.storage.as_slice(),
            self.weights_start + m.routing_offset,
            m.routing_size,
        )?))
    }

    /// Run a packed subatomic classifier over `text`. Returns `(class_idx, confidence)`.
    ///
    /// Mirrors [`crate::swarm::train::predict`]: trigram-hash featurize → linear → softmax.
    /// The packed weights blob is `[W (nc * fd) | b (nc)]`, row-major (`W[c * fd + d]`).
    pub fn infer(&self, model_name: &str, text: &str) -> Result<(usize, f32)> {
        let m = self.find(model_name)?;
        let nc = m.num_classes as usize;
        let fd = m.feature_dim as usize;
        let packed = self.weights(model_name)?;
        if packed.len() != nc * fd + nc {
            return Err(Error::BadManifest);
        }
        let (w, b) = packed.split_at(nc * fd);

        let feat = crate::swarm::train::featurize(text, fd);
        let mut logits = vec![0.0f32; nc];
        for c in 0..nc {
            let mut sum = b[c];
            let row = &w[c * fd..(c + 1) * fd];
            for d in 0..fd {
                sum += row[d] * feat[d];
            }
            logits[c] = sum;
        }

        let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut sum_exp = 0.0f32;
        for v in logits.iter_mut() {
            *v = (*v - max_logit).exp();
            sum_exp += *v;
        }
        for v in logits.iter_mut() {
            *v /= sum_exp;
        }

        let (idx, conf) = logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, p)| (i, *p))
            .unwrap_or((0, 0.0));
        Ok((idx, conf))
    }

    /// Run [`infer`](Self::infer) and resolve the class index to a name via [`STARTER_CLASS_NAMES`].
    /// Errors with [`Error::NotFound`] if the model isn't in the class-name table.
    pub fn infer_named(&self, model_name: &str, text: &str) -> Result<(&'static str, f32)> {
        let names = STARTER_CLASS_NAMES
            .iter()
            .find(|(n, _)| *n == model_name)
            .map(|(_, names)| *names)
            .ok_or_else(|| Error::NotFound(model_name.to_string()))?;
        let (idx, conf) = self.infer(model_name, text)?;
        let name = names.get(idx).copied().unwrap_or("?");
        Ok((name, conf))
    }
}

/// Class-name lookup for models packed in `assets/starter.nanobyte`.
///
/// Kept as a static map rather than embedded in the manifest so the on-disk
/// format stays version-stable. When the format gains a string-table region
/// (V2), this becomes derivable and can be removed.
pub const STARTER_CLASS_NAMES: &[(&str, &[&str])] = &[
    ("slop_detector", &["clean", "slop"]),
    ("code_vs_english", &["english", "code"]),
    ("lang_detector", &["rust", "python", "javascript", "go", "shell"]),
];

/// Bytes of `assets/starter.nanobyte` baked into the binary at compile time.
/// Zero file I/O at runtime — the slice lives in the binary's `.rodata`.
pub static STARTER_NANOBYTE: &[u8] = include_bytes!("../assets/starter.nanobyte");

/// Load the embedded starter nanobyte. Pure in-memory; no `fs` access.
pub fn starter() -> Result<Nanobyte> {
    Nanobyte::from_bytes(STARTER_NANOBYTE)
}

/// One model's classification of a piece of text. Persisted as REPL telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub model: String,
    pub class_idx: usize,
    pub class_name: String,
    pub confidence: f32,
}

/// Run every model in [`STARTER_CLASS_NAMES`] against `text` and collect results.
/// Models missing from the nanobyte are skipped silently; this is for telemetry,
/// not control flow.
pub fn classify_with_starters(nb: &Nanobyte, text: &str) -> Vec<Classification> {
    STARTER_CLASS_NAMES
        .iter()
        .filter_map(|(model, names)| {
            let (idx, conf) = nb.infer(model, text).ok()?;
            let class_name = names.get(idx).copied().unwrap_or("?").to_string();
            Some(Classification {
                model: (*model).to_string(),
                class_idx: idx,
                class_name,
                confidence: conf,
            })
        })
        .collect()
}

fn slice_f32(mmap: &[u8], offset: u64, size: u64) -> Result<&[f32]> {
    let start = offset as usize;
    let end = start
        .checked_add(size as usize)
        .ok_or(Error::Misaligned(offset))?;
    if end > mmap.len() {
        return Err(Error::BadManifest);
    }
    if !start.is_multiple_of(4) || !size.is_multiple_of(4) {
        return Err(Error::Misaligned(offset));
    }
    let bytes = &mmap[start..end];
    let ptr = bytes.as_ptr();
    if !(ptr as usize).is_multiple_of(std::mem::align_of::<f32>()) {
        return Err(Error::Misaligned(offset));
    }
    // SAFETY: alignment + length-divisible-by-4 verified above; in-bounds checked;
    // every f32 bit pattern is valid; lifetime tied to the mmap borrow.
    let floats: &[f32] =
        unsafe { std::slice::from_raw_parts(ptr as *const f32, bytes.len() / 4) };
    Ok(floats)
}

fn decode_manifest(b: &[u8]) -> Manifest {
    let name_bytes: &[u8] = &b[0..NAME_LEN];
    let name_end = name_bytes.iter().position(|&x| x == 0).unwrap_or(NAME_LEN);
    let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
    let tier = b[32];
    let num_classes = u32::from_le_bytes(b[36..40].try_into().unwrap());
    let feature_dim = u32::from_le_bytes(b[40..44].try_into().unwrap());
    let offset = u64::from_le_bytes(b[48..56].try_into().unwrap());
    let size = u64::from_le_bytes(b[56..64].try_into().unwrap());
    let routing_offset = u64::from_le_bytes(b[64..72].try_into().unwrap());
    let routing_size = u64::from_le_bytes(b[72..80].try_into().unwrap());
    Manifest {
        name,
        tier,
        num_classes,
        feature_dim,
        offset,
        size,
        routing_offset,
        routing_size,
    }
}

fn encode_manifest(m: &Manifest) -> [u8; MANIFEST_ENTRY_SIZE] {
    let mut buf = [0u8; MANIFEST_ENTRY_SIZE];
    let name_bytes = m.name.as_bytes();
    let n = name_bytes.len().min(NAME_LEN);
    buf[..n].copy_from_slice(&name_bytes[..n]);
    buf[32] = m.tier;
    buf[36..40].copy_from_slice(&m.num_classes.to_le_bytes());
    buf[40..44].copy_from_slice(&m.feature_dim.to_le_bytes());
    buf[48..56].copy_from_slice(&m.offset.to_le_bytes());
    buf[56..64].copy_from_slice(&m.size.to_le_bytes());
    buf[64..72].copy_from_slice(&m.routing_offset.to_le_bytes());
    buf[72..80].copy_from_slice(&m.routing_size.to_le_bytes());
    buf
}

/// Pack models into a `.nanobyte` file at `output`. Atomic via tmp-then-rename.
pub fn consolidate(models: &[PackInput<'_>], output: &Path) -> Result<()> {
    for m in models {
        if m.name.len() > NAME_LEN {
            return Err(Error::NameTooLong(m.name.to_string()));
        }
    }

    let num_models = models.len() as u32;
    let manifest_offset = HEADER_SIZE as u64;
    let manifest_size = (models.len() * MANIFEST_ENTRY_SIZE) as u64;

    let mut manifests = Vec::with_capacity(models.len());
    let mut cursor: u64 = 0;
    let mut total_weights: u64 = 0;
    for m in models {
        let weight_bytes = std::mem::size_of_val(m.weights) as u64;
        let routing_bytes = m
            .routing
            .map(|r| std::mem::size_of_val(r) as u64)
            .unwrap_or(0);
        let entry = Manifest {
            name: m.name.to_string(),
            tier: m.tier,
            num_classes: m.num_classes,
            feature_dim: m.feature_dim,
            offset: cursor,
            size: weight_bytes,
            routing_offset: if routing_bytes > 0 {
                cursor + weight_bytes
            } else {
                0
            },
            routing_size: routing_bytes,
        };
        manifests.push(entry);
        cursor += weight_bytes + routing_bytes;
        total_weights += weight_bytes + routing_bytes;
    }

    let mut header = [0u8; HEADER_SIZE];
    header[0..4].copy_from_slice(&MAGIC);
    header[4..8].copy_from_slice(&VERSION.to_le_bytes());
    header[8..12].copy_from_slice(&num_models.to_le_bytes());
    header[12..20].copy_from_slice(&manifest_offset.to_le_bytes());
    header[20..28].copy_from_slice(&manifest_size.to_le_bytes());
    header[28..36].copy_from_slice(&total_weights.to_le_bytes());

    let total_payload = HEADER_SIZE + manifest_size as usize + total_weights as usize;
    let mut buf: Vec<u8> = Vec::with_capacity(total_payload + NSIG_SIZE);
    buf.extend_from_slice(&header);
    for entry in &manifests {
        buf.extend_from_slice(&encode_manifest(entry));
    }
    for m in models {
        buf.extend_from_slice(weights_as_bytes(m.weights));
        if let Some(r) = m.routing {
            buf.extend_from_slice(weights_as_bytes(r));
        }
    }

    let hash = blake3::hash(&buf);
    buf.extend_from_slice(&NSIG_MAGIC);
    buf.extend_from_slice(hash.as_bytes());

    let tmp = output.with_extension("nanobyte.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&buf)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, output)?;
    Ok(())
}

fn weights_as_bytes(w: &[f32]) -> &[u8] {
    // SAFETY: f32 is `Copy`, has the same alignment-or-stricter than u8, and the
    // slice's lifetime is the same. Length scaled by size_of::<f32>().
    unsafe { std::slice::from_raw_parts(w.as_ptr() as *const u8, std::mem::size_of_val(w)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_two_models() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("test.nanobyte");

        let w1: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let w2: Vec<f32> = (0..32).map(|i| (i as f32) * 0.5).collect();
        let r2: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];

        let inputs = vec![
            PackInput {
                name: "first",
                tier: 1,
                num_classes: 2,
                feature_dim: 8,
                weights: &w1,
                routing: None,
            },
            PackInput {
                name: "second",
                tier: 2,
                num_classes: 4,
                feature_dim: 8,
                weights: &w2,
                routing: Some(&r2),
            },
        ];
        consolidate(&inputs, &out).unwrap();

        let nb = Nanobyte::load(&out).unwrap();
        assert_eq!(nb.manifests().len(), 2);
        assert_eq!(nb.manifests()[0].name, "first");
        assert_eq!(nb.manifests()[0].tier, 1);
        assert_eq!(nb.manifests()[1].name, "second");
        assert_eq!(nb.manifests()[1].tier, 2);
        assert_eq!(nb.weights("first").unwrap(), &w1[..]);
        assert_eq!(nb.weights("second").unwrap(), &w2[..]);
        assert_eq!(nb.routing("first").unwrap(), None);
        assert_eq!(nb.routing("second").unwrap(), Some(&r2[..]));
    }

    #[test]
    fn detects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("tampered.nanobyte");
        let w: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        consolidate(
            &[PackInput {
                name: "x",
                tier: 1,
                num_classes: 2,
                feature_dim: 4,
                weights: &w,
                routing: None,
            }],
            &out,
        )
        .unwrap();

        let mut data = fs::read(&out).unwrap();
        let idx = HEADER_SIZE + MANIFEST_ENTRY_SIZE + 4;
        data[idx] ^= 0xFF;
        fs::write(&out, &data).unwrap();

        match Nanobyte::load(&out) {
            Err(Error::BadSignature) => {}
            Err(e) => panic!("expected BadSignature, got {e:?}"),
            Ok(_) => panic!("expected BadSignature, got Ok"),
        }
    }

    #[test]
    fn rejects_unsigned() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("unsigned.nanobyte");
        let w: Vec<f32> = vec![1.0, 2.0];
        consolidate(
            &[PackInput {
                name: "y",
                tier: 1,
                num_classes: 2,
                feature_dim: 2,
                weights: &w,
                routing: None,
            }],
            &out,
        )
        .unwrap();
        let mut data = fs::read(&out).unwrap();
        let n = data.len();
        data[n - NSIG_SIZE..n - NSIG_SIZE + 4].copy_from_slice(b"XXXX");
        fs::write(&out, &data).unwrap();

        match Nanobyte::load(&out) {
            Err(Error::Unsigned) => {}
            Err(e) => panic!("expected Unsigned, got {e:?}"),
            Ok(_) => panic!("expected Unsigned, got Ok"),
        }
    }

    #[test]
    fn rejects_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("badmagic.nanobyte");
        let w: Vec<f32> = vec![1.0, 2.0];
        consolidate(
            &[PackInput {
                name: "z",
                tier: 1,
                num_classes: 2,
                feature_dim: 2,
                weights: &w,
                routing: None,
            }],
            &out,
        )
        .unwrap();
        let mut data = fs::read(&out).unwrap();
        data[0..4].copy_from_slice(b"OOPS");
        // Re-sign so signature passes; magic check should still fail.
        let payload_end = data.len() - NSIG_SIZE;
        let new_hash = blake3::hash(&data[..payload_end]);
        data[payload_end..payload_end + 4].copy_from_slice(&NSIG_MAGIC);
        data[payload_end + 4..].copy_from_slice(new_hash.as_bytes());
        fs::write(&out, &data).unwrap();

        match Nanobyte::load(&out) {
            Err(Error::BadMagic(m)) => assert_eq!(&m, b"OOPS"),
            Err(e) => panic!("expected BadMagic, got {e:?}"),
            Ok(_) => panic!("expected BadMagic, got Ok"),
        }
    }

    #[test]
    fn weights_region_4_aligned_for_any_n() {
        for n in 0..=16 {
            assert_eq!((HEADER_SIZE + n * MANIFEST_ENTRY_SIZE) % 4, 0);
        }
    }

    fn starter_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/starter.nanobyte")
    }

    #[test]
    fn starter_loads_three_models() {
        let path = starter_path();
        if !path.exists() {
            eprintln!("skipping: {} missing — run `cargo run --bin pack-starter`", path.display());
            return;
        }
        let nb = Nanobyte::load(&path).expect("starter.nanobyte must verify");
        let names: Vec<&str> = nb.manifests().iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            ["slop_detector", "code_vs_english", "lang_detector"]
        );
    }

    /// Parity contract: nanobyte::infer must produce the same class index and
    /// near-identical confidence as swarm/train::predict on the same model + text.
    /// This is the real correctness test — the absolute classification depends on
    /// model accuracy (94% / 89% / 97%) and is brittle to test directly.
    #[test]
    fn infer_matches_swarm_predict_for_all_starters() {
        let path = starter_path();
        let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/models");
        if !path.exists() {
            return;
        }
        let nb = Nanobyte::load(&path).unwrap();

        let probes = [
            "fn main() { println!(\"hello\"); }",
            "The quick brown fox jumps over the lazy dog.",
            "def main():\n    print(\"hi\")",
            "let xs: Vec<u32> = (0..10).collect();",
            "import os\nprint(os.getcwd())",
            "package main\nimport \"fmt\"\nfunc main() { fmt.Println(\"hi\") }",
        ];

        for model in ["slop_detector", "code_vs_english", "lang_detector"] {
            let on_disk = assets.join(model);
            if !on_disk.exists() {
                continue;
            }
            for text in probes {
                let (nb_idx, nb_conf) = nb.infer(model, text).unwrap();
                let (sw_idx, _sw_name, sw_conf) =
                    crate::swarm::train::predict(&on_disk, text).unwrap();
                assert_eq!(
                    nb_idx, sw_idx,
                    "{model}: nanobyte idx {nb_idx} != swarm idx {sw_idx} for {text:?}"
                );
                assert!(
                    (nb_conf - sw_conf).abs() < 1e-5,
                    "{model}: nanobyte conf {nb_conf} differs from swarm {sw_conf} for {text:?}"
                );
            }
        }
    }

    #[test]
    fn infer_named_resolves_class_strings() {
        let path = starter_path();
        if !path.exists() {
            return;
        }
        let nb = Nanobyte::load(&path).unwrap();
        let (name, conf) = nb
            .infer_named("slop_detector", "let x = 1;")
            .expect("slop_detector must be in STARTER_CLASS_NAMES");
        assert!(["clean", "slop"].contains(&name), "got {name:?}");
        assert!((0.0..=1.0).contains(&conf));
    }

    #[test]
    fn embedded_starter_loads() {
        let nb = starter().expect("STARTER_NANOBYTE must verify at startup");
        let names: Vec<&str> = nb.manifests().iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            ["slop_detector", "code_vs_english", "lang_detector"]
        );
    }

    /// The embedded bytes and the on-disk file must be byte-identical (the
    /// embed is generated from the file at build time). If they diverge, the
    /// pack-starter binary needs to be re-run before this commit.
    #[test]
    fn embedded_matches_on_disk() {
        let path = starter_path();
        if !path.exists() {
            return;
        }
        let on_disk = std::fs::read(&path).unwrap();
        assert_eq!(STARTER_NANOBYTE, on_disk.as_slice());
    }

    #[test]
    fn classify_with_starters_returns_one_per_model() {
        let nb = starter().unwrap();
        let out = classify_with_starters(&nb, "fn main() { println!(\"hi\"); }");
        assert_eq!(out.len(), STARTER_CLASS_NAMES.len());
        let models: Vec<&str> = out.iter().map(|c| c.model.as_str()).collect();
        assert_eq!(
            models,
            ["slop_detector", "code_vs_english", "lang_detector"]
        );
        for c in &out {
            assert!((0.0..=1.0).contains(&c.confidence), "{c:?}");
            assert!(!c.class_name.is_empty());
        }
    }

    #[test]
    fn embedded_infer_matches_mmap_infer() {
        let path = starter_path();
        if !path.exists() {
            return;
        }
        let mmapped = Nanobyte::load(&path).unwrap();
        let embedded = starter().unwrap();
        for text in [
            "fn main() { let x = 1; }",
            "the cat sat on the mat",
            "import os; os.path.join('a', 'b')",
        ] {
            for model in ["slop_detector", "code_vs_english", "lang_detector"] {
                let m = mmapped.infer(model, text).unwrap();
                let e = embedded.infer(model, text).unwrap();
                assert_eq!(m.0, e.0, "{model} idx differs for {text:?}");
                assert!(
                    (m.1 - e.1).abs() < 1e-6,
                    "{model} conf differs for {text:?}: mmap {} vs embedded {}",
                    m.1,
                    e.1
                );
            }
        }
    }
}
