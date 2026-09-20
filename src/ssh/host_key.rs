use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use russh::keys::{Algorithm, PrivateKey};

/// Load the server identity, creating it atomically on the first run.
/// Existing unreadable, encrypted or invalid keys are never replaced.
pub fn load_or_create_host_key(path: &Path) -> io::Result<PrivateKey> {
    match load(path) {
        Ok(key) => return Ok(key),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).map_err(io::Error::other)?;
    let encoded = key
        .to_openssh(russh::keys::ssh_key::LineEnding::LF)
        .map_err(io::Error::other)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = parent.join(format!(
        ".chessh-host-key-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    let result = (|| {
        file.write_all(encoded.as_bytes())?;
        file.sync_all()?;
        // A hard link publishes the complete key without replacing a concurrent creator.
        match fs::hard_link(&temp, path) {
            Ok(()) => Ok(key),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => load(path),
            Err(error) => Err(error),
        }
    })();
    drop(file);
    let cleanup = fs::remove_file(&temp);
    if let Err(error) = cleanup {
        tracing::warn!("Could not remove temporary host key: {error}");
    }
    result
}

fn load(path: &Path) -> io::Result<PrivateKey> {
    let encoded = fs::read_to_string(path)?;
    let key = PrivateKey::from_openssh(encoded)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if key.is_encrypted() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Host key must be unencrypted",
        ));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_survives_restart_and_has_private_permissions() {
        let path = std::env::temp_dir().join(format!("chessh-key-test-{}", rand::random::<u64>()));
        let first = load_or_create_host_key(&path).unwrap();
        let bytes = fs::read(&path).unwrap();
        let second = load_or_create_host_key(&path).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(bytes, fs::read(&path).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_existing_key_is_not_replaced() {
        let path = std::env::temp_dir().join(format!("chessh-key-test-{}", rand::random::<u64>()));
        fs::write(&path, "invalid key").unwrap();
        assert!(load_or_create_host_key(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid key");
        fs::remove_file(path).unwrap();
    }
}
