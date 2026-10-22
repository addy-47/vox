use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use zeroize::Zeroizing;

use super::{
    errors::SecurityError,
    identity::{DeviceKey, X25519_KEY_BYTES},
};
use crate::utils::paths;

/// The version byte of the key file format.
///
/// The file is one version byte followed by the private half. A file of any
/// other length, or with an unknown version byte, is an error — never a
/// guess.
pub const KEY_FILE_VERSION: u8 = 1;

/// The number of bytes a key file holds: the version byte and the private key.
pub const KEY_FILE_BYTES: usize = 1 + X25519_KEY_BYTES;

/// The file mode a key file must carry.
pub const KEY_FILE_MODE: u32 = 0o600;

/// The suffix of the temporary file the atomic write uses.
const KEY_FILE_TEMP_SUFFIX: &str = ".tmp";

/// The store of the device's private key half.
///
/// The private half is **never** in the database: the database is exported
/// and backed up. The file holds the private half only; the public half is
/// derived on load, so a stored pair can never disagree with itself.
pub struct KeyStore {
    path: PathBuf,
}

impl KeyStore {
    /// A store at an explicit path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The store at the platform's device key path.
    pub fn default_path() -> Result<Self, SecurityError> {
        let paths = paths::try_get().ok_or_else(|| SecurityError::KeyStoreIo {
            action: "located",
            source: io::Error::new(
                io::ErrorKind::NotFound,
                "the storage paths have not been initialised",
            ),
        })?;
        Ok(Self::new(paths.device_key))
    }

    /// The path of the key file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the device key, creating one when the file does not exist yet.
    ///
    /// The file is the device identity. If another process created it first,
    /// the file's key wins, so the key is read back rather than returned from
    /// memory — two processes must never disagree about who this device is.
    pub fn load_or_create(&self) -> Result<DeviceKey, SecurityError> {
        match self.load() {
            Ok(key) => return Ok(key),
            Err(SecurityError::KeyStoreIo { source, .. })
                if source.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let key = DeviceKey::generate()?;
        self.save(&key)?;
        self.load()
    }

    /// Loads the device key from the file.
    pub fn load(&self) -> Result<DeviceKey, SecurityError> {
        let bytes = fs::read(&self.path).map_err(|source| SecurityError::KeyStoreIo {
            action: "read",
            source,
        })?;
        // The buffer holds the private half. It is zeroized on drop so it
        // does not linger in the heap after the key has been rebuilt.
        let bytes = Zeroizing::new(bytes);
        self.decode(bytes.as_slice())
    }

    /// Writes the device key to the file.
    ///
    /// The write is atomic: the bytes go to a temporary file in the same
    /// directory and are then renamed over the target, so a crash cannot
    /// leave a truncated key behind.
    pub fn save(&self, key: &DeviceKey) -> Result<(), SecurityError> {
        let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_FILE_BYTES));
        bytes.push(KEY_FILE_VERSION);
        bytes.extend_from_slice(key.private_key_bytes());

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| SecurityError::KeyStoreIo {
                action: "created the directory of",
                source,
            })?;
        }

        let temporary = self.temporary_path();
        self.write_private(&temporary, &bytes)?;
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(SecurityError::KeyStoreIo {
                action: "renamed into place",
                source: error,
            });
        }
        self.sync_parent_directory();
        Ok(())
    }

    /// Flushes the parent directory so the rename survives a crash.
    ///
    /// A rename is atomic, but it is not durable until the directory entry
    /// itself has been flushed.
    fn sync_parent_directory(&self) {
        #[cfg(unix)]
        if let Some(parent) = self.path.parent() {
            if let Ok(directory) = fs::File::open(parent) {
                let _ = directory.sync_all();
            }
        }
    }

    /// Decodes the bytes of a key file into a device key.
    fn decode(&self, bytes: &[u8]) -> Result<DeviceKey, SecurityError> {
        if bytes.len() != KEY_FILE_BYTES {
            return Err(SecurityError::KeyStoreFormat(format!(
                "the key file must be {KEY_FILE_BYTES} bytes, got {}",
                bytes.len()
            )));
        }
        if bytes[0] != KEY_FILE_VERSION {
            return Err(SecurityError::KeyStoreFormat(format!(
                "unknown key file version {}",
                bytes[0]
            )));
        }
        self.tighten_permissions()?;
        DeviceKey::from_private_bytes(bytes[1..].to_vec())
    }

    /// Writes bytes to a private file: mode 0600, and never wider.
    fn write_private(&self, path: &Path, bytes: &[u8]) -> Result<(), SecurityError> {
        let _ = fs::remove_file(path);
        let mut file = create_private(path).map_err(|source| SecurityError::KeyStoreIo {
            action: "created",
            source,
        })?;
        file.write_all(bytes)
            .map_err(|source| SecurityError::KeyStoreIo {
                action: "wrote",
                source,
            })?;
        file.sync_all()
            .map_err(|source| SecurityError::KeyStoreIo {
                action: "flushed",
                source,
            })?;
        drop(file);
        set_private_mode(path)?;
        Ok(())
    }

    /// Tightens the file mode when it is wider than 0600.
    ///
    /// A wide mode is a warning, not a reason to refuse to start: refusing
    /// to launch the app because of a file mode is a worse failure than the
    /// mode itself.
    fn tighten_permissions(&self) -> Result<(), SecurityError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let metadata =
                fs::metadata(&self.path).map_err(|source| SecurityError::KeyStoreIo {
                    action: "read the mode of",
                    source,
                })?;
            let mode = metadata.permissions().mode() & 0o777;
            if mode & !KEY_FILE_MODE != 0 {
                log::warn!(
                    "[Remote::Security] The device key file mode is wider than 0600; tightening it \
                     mode={mode:o}"
                );
                set_private_mode(&self.path)?;
            }
        }
        Ok(())
    }

    /// The path of the temporary file the atomic write uses.
    fn temporary_path(&self) -> PathBuf {
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("device.key");
        let temporary = format!("{file_name}{KEY_FILE_TEMP_SUFFIX}");
        match self.path.parent() {
            Some(parent) => parent.join(temporary),
            None => PathBuf::from(temporary),
        }
    }
}

/// Creates a file that only its owner may read and write.
fn create_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(KEY_FILE_MODE);
    }
    options.open(path)
}

/// Sets a file's mode to exactly 0600.
#[cfg(unix)]
fn set_private_mode(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(KEY_FILE_MODE)).map_err(|source| {
        SecurityError::KeyStoreIo {
            action: "set the mode of",
            source,
        }
    })
}

/// Sets a file's mode to exactly 0600.
#[cfg(not(unix))]
fn set_private_mode(_path: &Path) -> Result<(), SecurityError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn key_store_creates_file_with_strict_mode_and_roundtrips() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("device.key");
        let store = KeyStore::new(path.clone());

        let key = store.load_or_create().expect("load or create");
        assert!(path.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, KEY_FILE_MODE);
        }

        let reloaded = store.load().expect("reload");
        assert_eq!(reloaded.public_key(), key.public_key());
        assert_eq!(reloaded.device_id().as_str(), key.device_id().as_str());

        // Validate on-disk binary format: version byte + 32-byte private key
        let on_disk = fs::read(&path).unwrap();
        assert_eq!(on_disk.len(), KEY_FILE_BYTES);
        assert_eq!(on_disk[0], KEY_FILE_VERSION);
        assert_eq!(&on_disk[1..], key.private_key_bytes());
    }

    #[test]
    fn key_store_rejects_corrupted_format() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("device.key");
        let store = KeyStore::new(path.clone());

        // Wrong length
        fs::write(&path, vec![0u8; KEY_FILE_BYTES - 1]).unwrap();
        assert!(matches!(
            store.load(),
            Err(SecurityError::KeyStoreFormat(_))
        ));

        // Unknown version
        let mut bad_version = vec![KEY_FILE_VERSION + 1];
        bad_version.extend_from_slice(&[0u8; X25519_KEY_BYTES]);
        fs::write(&path, bad_version).unwrap();
        assert!(matches!(
            store.load(),
            Err(SecurityError::KeyStoreFormat(_))
        ));
    }

    #[test]
    fn key_file_is_replaced_atomically_via_rename() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("device.key");
        let store = KeyStore::new(path.clone());

        let first = store.load_or_create().expect("create first");
        let second = DeviceKey::generate().expect("generate second");

        #[cfg(unix)]
        let first_inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&path).unwrap().ino()
        };

        store.save(&second).expect("save second");

        #[cfg(unix)]
        let second_inode = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&path).unwrap().ino()
        };

        #[cfg(unix)]
        assert_ne!(
            first_inode, second_inode,
            "file must be replaced via atomic rename"
        );

        let reloaded = store.load().expect("reload");
        assert_eq!(reloaded.public_key(), second.public_key());
        assert_ne!(reloaded.public_key(), first.public_key());
    }
}
