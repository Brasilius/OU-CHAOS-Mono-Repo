use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_FILE_BYTES: u64 = 95 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct QueuedFile {
    pub path: PathBuf,
    pub bytes: u64,
    pub kind: String,
}

impl QueuedFile {
    pub fn inspect(path: PathBuf) -> Result<Self> {
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Cannot read {}", path.display()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!(
                "Select regular files, not directories or symbolic links: {}",
                path.display()
            );
        }
        if metadata.len() > MAX_FILE_BYTES {
            bail!(
                "{} exceeds 95 MiB. Use a separate Git LFS workflow for large results.",
                path.display()
            );
        }
        let path = path.canonicalize()?;
        let kind = classify(&path).to_owned();
        Ok(Self {
            path,
            bytes: metadata.len(),
            kind,
        })
    }
}

pub fn classify(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "ork" | "rkt" => "openrocket",
        "rse" | "eng" => "propulsion",
        "foam" | "cas" | "msh" | "cgns" | "vtk" | "vtu" | "vtp" | "cfx" => "cfd",
        "stl" | "step" | "stp" | "iges" | "igs" | "obj" => "geometry",
        "csv" | "tsv" | "dat" | "h5" | "hdf5" | "mat" | "sim" => "results",
        "png" | "jpg" | "jpeg" | "svg" | "pdf" => "review",
        _ => "other",
    }
}

pub fn safe_filename(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    let clean = clean.trim_matches('.');
    if clean.is_empty() {
        "file".into()
    } else {
        clean.into()
    }
}

#[derive(Serialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub version: String,
    pub created_at: String,
    pub project: String,
    pub author: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub files: Vec<FileRecord>,
}

#[derive(Serialize)]
pub struct FileRecord {
    pub original_name: String,
    pub path: String,
    pub kind: String,
    pub bytes: u64,
    pub sha256: String,
}

pub fn build_batch(
    staging: &Path,
    project: &str,
    author: &str,
    notes: &str,
    tags: &str,
    files: &[QueuedFile],
) -> Result<Manifest> {
    if files.is_empty() {
        bail!("Add at least one file before uploading.");
    }
    fs::create_dir(staging).context("Cannot create unique staging directory")?;
    // Simulation files must remain byte-identical to their checksum manifest,
    // even when a user's Git normally normalizes CRLF or applies LFS filters.
    fs::write(
        staging.join(".gitattributes"),
        "# Preserve archived simulation bytes on every platform.\n* -text -filter -ident -working-tree-encoding\n",
    )?;
    let now = Utc::now();
    let version = now.format("%Y%m%dT%H%M%S%.9fZ").to_string();
    let mut manifest = Manifest {
        schema_version: 1,
        version,
        created_at: now.to_rfc3339(),
        project: project.into(),
        author: author.into(),
        notes: notes.into(),
        tags: tags
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .collect(),
        files: Vec::new(),
    };
    for (index, queued) in files.iter().enumerate() {
        let checked = QueuedFile::inspect(queued.path.clone())?;
        let original = checked
            .path
            .file_name()
            .context("File has no name")?
            .to_string_lossy()
            .into_owned();
        let relative = format!(
            "{}/{:03}-{}",
            checked.kind,
            index + 1,
            safe_filename(&original)
        );
        let destination = staging.join(&relative);
        fs::create_dir_all(destination.parent().context("Invalid destination")?)?;
        let mut input = File::open(&checked.path)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        let mut hash = Sha256::new();
        let mut bytes = 0;
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            if bytes > MAX_FILE_BYTES {
                bail!("File grew beyond the 95 MiB limit while copying.");
            }
            output.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
        }
        output.sync_all()?;
        manifest.tags.push(checked.kind.clone());
        manifest.files.push(FileRecord {
            original_name: original,
            path: relative,
            kind: checked.kind,
            bytes,
            sha256: format!("{:x}", hash.finalize()),
        });
    }
    manifest.tags.sort();
    manifest.tags.dedup();
    fs::write(
        staging.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// Terminals send file drops as text: quoted paths, escaped spaces, or file:// URIs.
pub fn dropped_paths(input: &str) -> Result<Vec<PathBuf>> {
    let input = input.trim();
    if Path::new(input).is_file() {
        return Ok(vec![PathBuf::from(input)]);
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' if quote == Some(c) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(c),
            '\\' if quote != Some('\'')
                && chars
                    .peek()
                    .is_some_and(|n| n.is_whitespace() || *n == '\'' || *n == '"') =>
            {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() && quote.is_none() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            _ => word.push(c),
        }
    }
    if quote.is_some() {
        bail!("Unclosed quote in path. Use Browse to select the file.");
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
        .into_iter()
        .map(|word| {
            if let Some(uri) = word.strip_prefix("file://") {
                let path = if let Some(rest) = uri.strip_prefix("localhost/") {
                    format!("/{rest}")
                } else if uri.starts_with('/') {
                    uri.to_owned()
                } else {
                    bail!("Only local file:// paths are supported.");
                };
                let bytes = path.as_bytes();
                let mut decoded = Vec::new();
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'%' {
                        let escape = bytes.get(i + 1..i + 3).context("Invalid file URI escape")?;
                        let escape =
                            std::str::from_utf8(escape).context("Invalid file URI escape")?;
                        decoded.push(
                            u8::from_str_radix(escape, 16).context("Invalid file URI escape")?,
                        );
                        i += 3;
                    } else {
                        decoded.push(bytes[i]);
                        i += 1;
                    }
                }
                let decoded = String::from_utf8(decoded)?;
                #[cfg(windows)]
                let decoded = if decoded.as_bytes().get(2) == Some(&b':') {
                    decoded[1..].to_owned()
                } else {
                    decoded
                };
                Ok(PathBuf::from(decoded))
            } else if let Some(home_relative) = word.strip_prefix("~/") {
                let home = directories::BaseDirs::new().context("Cannot find home directory")?;
                Ok(home.home_dir().join(home_relative))
            } else {
                Ok(PathBuf::from(word))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_terminal_drop_formats() {
        assert_eq!(
            dropped_paths("'/tmp/a b.ork' /tmp/c\\ d.csv").unwrap(),
            vec![PathBuf::from("/tmp/a b.ork"), PathBuf::from("/tmp/c d.csv")]
        );
        assert_eq!(
            dropped_paths(r#""C:\Users\Pilot\a b.ork""#).unwrap(),
            vec![PathBuf::from(r"C:\Users\Pilot\a b.ork")]
        );
        #[cfg(not(windows))]
        assert_eq!(
            dropped_paths("file:///tmp/a%20b.ork").unwrap(),
            vec![PathBuf::from("/tmp/a b.ork")]
        );
        assert!(dropped_paths("'unterminated").is_err());
        assert!(dropped_paths("file:///tmp/%").is_err());
    }
    #[test]
    fn batch_preserves_data_and_records_checksums_without_collisions() {
        let tmp = tempfile::tempdir().unwrap();
        let first = tmp.path().join("A.ork");
        let second = tmp.path().join("a.ork");
        fs::write(&first, b"abc").unwrap();
        fs::write(&second, b"abc").unwrap();
        let files = vec![
            QueuedFile::inspect(first.clone()).unwrap(),
            QueuedFile::inspect(second).unwrap(),
        ];
        let stage = tmp.path().join("stage");
        let manifest = build_batch(
            &stage,
            "rocket",
            "Pilot",
            "review",
            "Mach-2, openrocket",
            &files,
        )
        .unwrap();
        assert_eq!(
            manifest.files[0].sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(manifest.files[0].path, manifest.files[1].path);
        assert_eq!(
            fs::read(stage.join(&manifest.files[0].path)).unwrap(),
            b"abc"
        );
        assert!(first.exists());
        assert_eq!(manifest.tags, ["mach-2", "openrocket"]);
    }
    #[test]
    fn rejects_large_files_and_directories() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(QueuedFile::inspect(tmp.path().into()).is_err());
        let path = tmp.path().join("large.vtk");
        File::create(&path)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        assert!(QueuedFile::inspect(path).is_err());
    }
}
