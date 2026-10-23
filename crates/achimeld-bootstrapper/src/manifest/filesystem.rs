//! Filesystem inventory and ownership of session-long package replacement locks.
use super::{MAX_MANIFEST, TrustConfig, VerifiedPackage, parse_and_authenticate};
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
};
fn read_locked(path: &Path, maximum: usize) -> Result<(File, Vec<u8>), String> {
    reject_reparse(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    options.share_mode(1).custom_flags(0x0020_0000);
    let mut file = options
        .open(path)
        .map_err(|_| "package file missing or not lockable")?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "package file unreadable")?;
    if bytes.len() > maximum {
        return Err("package file exceeds limit".into());
    }
    Ok((file, bytes))
}

pub fn reject_reparse(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("package/profile reparse point rejected".into());
                }
            }
            if metadata.file_type().is_symlink() {
                return Err("symbolic link rejected".into());
            }
        }
    }
    Ok(())
}

pub fn verify(root: &Path) -> Result<VerifiedPackage, String> {
    reject_reparse(root)?;
    let trust: TrustConfig =
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/trusted.json")))
            .map_err(|_| "invalid embedded trust configuration")?;
    let (manifest_lock, bytes) = read_locked(&root.join("manifest.json"), MAX_MANIFEST)?;
    let (signature_lock, signature) = read_locked(&root.join("manifest.sig"), 8192)?;
    let signature = STANDARD
        .decode(signature)
        .map_err(|_| "invalid manifest signature encoding")?;
    let manifest = parse_and_authenticate(&bytes, &signature, &trust)?;
    let mut locks = vec![manifest_lock, signature_lock];
    let mut expected = HashSet::new();
    expected.extend(["manifest.json".into(), "manifest.sig".into()]);
    for item in &manifest.files {
        let path = root.join(&item.path);
        reject_reparse(&path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        options.share_mode(1).custom_flags(0x0020_0000);
        let file = options
            .open(&path)
            .map_err(|_| "required package file cannot be locked")?;
        if file
            .metadata()
            .map_err(|_| "cannot inspect package file")?
            .len()
            != item.size
        {
            return Err("package file size mismatch".into());
        }
        let mut hash = Sha256::new();
        let mut reader = &file;
        let mut buffer = [0; 65536];
        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|_| "cannot hash package file")?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if !hex::encode(hash.finalize()).eq_ignore_ascii_case(&item.sha256) {
            return Err("package file hash mismatch".into());
        }
        expected.insert(item.path.to_ascii_lowercase());
        locks.push(file);
    }
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|_| "cannot inventory installation")? {
            let entry = entry.map_err(|_| "invalid installation entry")?;
            reject_reparse(&entry.path())?;
            if entry
                .file_type()
                .map_err(|_| "cannot inspect entry")?
                .is_dir()
            {
                directories.push(entry.path());
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase();
                if !expected.contains(&relative) {
                    return Err("unapproved package file rejected".into());
                }
            }
        }
        #[cfg(windows)]
        {
            let lock = OpenOptions::new()
                .read(true)
                .share_mode(1 | 2)
                .custom_flags(0x0200_0000 | 0x0020_0000)
                .open(&directory)
                .map_err(|_| "cannot lock installation directory")?;
            locks.push(lock);
        }
    }
    Ok(VerifiedPackage {
        root: root.to_owned(),
        manifest,
        _locks: locks,
    })
}
