use std::path::Path;

use sysinfo::Disks;

/// Returns the free bytes on the filesystem backing `path`.
///
/// Picks the disk whose mount point is the longest prefix of `path`, so a
/// models directory on a mounted volume resolves to that volume rather than to
/// `/`. Returns `None` when no mounted disk encloses the path — callers decide
/// whether that blocks them.
pub fn available_space_at(path: &Path) -> Option<u64> {
    let disks = Disks::new_with_refreshed_list();
    let mut best_match: Option<(&Path, u64)> = None;

    for disk in &disks {
        let mount = disk.mount_point();
        if !path.starts_with(mount) {
            continue;
        }
        let better_fit = match best_match {
            Some((best_mount, _)) => mount.as_os_str().len() > best_mount.as_os_str().len(),
            None => true,
        };
        if better_fit {
            best_match = Some((mount, disk.available_space()));
        }
    }

    best_match.map(|(_, available)| available)
}
