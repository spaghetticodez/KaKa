//! Reset of the state FLiNG trainers keep inside a game's Proton prefix.
//!
//! FLiNG trainers refuse to work ("previous trainer instance did not exit
//! properly") when stale state is left in `%LocalAppData%\FLiNGTrainer` or
//! `%LocalAppData%\Temp` (upstream LaLa #78; clearing both fixed it). Nothing
//! else is touched; in particular not Proton's `pfx.lock`, which is a flock
//! lock whose file may exist at any time.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum CleanupError {
    /// The game or a trainer still runs in the prefix.
    PrefixInUse,
    /// `%LocalAppData%` resolves to a location outside the prefix.
    OutsidePrefix,
    Io(io::Error),
}

impl fmt::Display for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrefixInUse => write!(f, "the prefix is in use"),
            Self::OutsidePrefix => write!(f, "AppData/Local points outside the prefix"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CleanupError {}

impl From<io::Error> for CleanupError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

pub struct TrainerDataCleanup {
    /// The game's prefix (`.../compatdata/<id>`), as recorded at launch.
    compat_data_path: PathBuf,
}

impl TrainerDataCleanup {
    pub fn new(compat_data_path: impl Into<PathBuf>) -> Self {
        Self {
            compat_data_path: compat_data_path.into(),
        }
    }

    /// `%LocalAppData%` of the Proton user.
    pub fn local_app_data(&self) -> PathBuf {
        self.compat_data_path
            .join("pfx/drive_c/users/steamuser/AppData/Local")
    }

    /// FLiNG's own folder: removed completely.
    pub fn fling_dir(&self) -> PathBuf {
        self.local_app_data().join("FLiNGTrainer")
    }

    /// Shared temp folder: emptied, the folder itself stays.
    pub fn temp_dir(&self) -> PathBuf {
        self.local_app_data().join("Temp")
    }

    pub fn targets(&self) -> [PathBuf; 2] {
        [self.fling_dir(), self.temp_dir()]
    }

    /// Deletes the targets and returns how many top-level items were removed.
    ///
    /// Refuses when `in_use` (game or trainer still running in the prefix) or
    /// when `%LocalAppData%` resolves outside the prefix. Symlinks are removed
    /// as links; their targets are never followed.
    pub fn run(&self, in_use: bool) -> Result<usize, CleanupError> {
        if in_use {
            return Err(CleanupError::PrefixInUse);
        }
        let local = self.local_app_data();
        if !local.is_dir() {
            return Ok(0);
        }
        let prefix_real = fs::canonicalize(&self.compat_data_path)?;
        let local_real = fs::canonicalize(&local)?;
        if local_real == prefix_real || !local_real.starts_with(&prefix_real) {
            return Err(CleanupError::OutsidePrefix);
        }

        let mut removed = 0;
        let fling = self.fling_dir();
        if fs::symlink_metadata(&fling).is_ok() {
            delete_entry(&fling)?;
            removed += 1;
        }

        let temp = self.temp_dir();
        if fs::symlink_metadata(&temp).is_ok_and(|m| m.is_dir()) {
            for entry in fs::read_dir(&temp)?.filter_map(Result::ok) {
                // Best effort: an item that cannot be removed does not block the rest.
                if delete_entry(&entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

/// Removes a file, a symlink (the link itself) or a directory tree.
/// `remove_dir_all` does not follow symlinks inside the tree.
fn delete_entry(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    struct Fixture {
        _tmp: TempDir,
        prefix: PathBuf,
        local: PathBuf,
        outside: PathBuf,
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    fn exists(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok()
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::Builder::new()
            .prefix("kaka_cleanup_")
            .tempdir()
            .unwrap();
        let prefix = tmp.path().join("compatdata/1245620");
        let local = prefix.join("pfx/drive_c/users/steamuser/AppData/Local");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        touch(&outside.join("precious.txt"));
        touch(&prefix.join("pfx.lock"));
        touch(&prefix.join("version"));
        Fixture {
            _tmp: tmp,
            prefix,
            local,
            outside,
        }
    }

    #[test]
    fn removes_fling_trainer_empties_temp_and_touches_nothing_else() {
        let f = fixture();
        touch(&f.local.join("FLiNGTrainer/state.dat"));
        touch(&f.local.join("FLiNGTrainer/sub/x"));
        touch(&f.local.join("Temp/a.tmp"));
        touch(&f.local.join("Temp/dir/b"));
        symlink(&f.outside, f.local.join("Temp/escape")).unwrap();
        symlink(&f.outside, f.local.join("Temp/dir/nested-escape")).unwrap();
        touch(&f.local.join("GameSaves/save.sav"));

        assert_eq!(TrainerDataCleanup::new(&f.prefix).run(false).unwrap(), 4);

        assert!(!exists(&f.local.join("FLiNGTrainer")));
        assert!(f.local.join("Temp").is_dir(), "Temp itself stays");
        assert_eq!(fs::read_dir(f.local.join("Temp")).unwrap().count(), 0);
        assert!(f.local.join("GameSaves/save.sav").exists());
        assert!(
            f.prefix.join("pfx.lock").exists(),
            "pfx.lock is never touched"
        );
        assert!(f.prefix.join("version").exists());
        assert!(
            f.outside.join("precious.txt").exists(),
            "symlink targets are never followed"
        );
    }

    #[test]
    fn refuses_while_the_prefix_is_in_use_and_deletes_nothing() {
        let f = fixture();
        touch(&f.local.join("FLiNGTrainer/state.dat"));
        touch(&f.local.join("Temp/a.tmp"));
        assert!(matches!(
            TrainerDataCleanup::new(&f.prefix).run(true),
            Err(CleanupError::PrefixInUse)
        ));
        assert!(f.local.join("FLiNGTrainer/state.dat").exists());
        assert!(f.local.join("Temp/a.tmp").exists());
    }

    #[test]
    fn refuses_when_appdata_local_resolves_outside_the_prefix() {
        let f = fixture();
        touch(&f.outside.join("FLiNGTrainer/state.dat"));
        touch(&f.outside.join("Temp/a.tmp"));
        fs::create_dir_all(f.local.parent().unwrap()).unwrap();
        symlink(&f.outside, &f.local).unwrap();

        assert!(matches!(
            TrainerDataCleanup::new(&f.prefix).run(false),
            Err(CleanupError::OutsidePrefix)
        ));
        assert!(f.outside.join("FLiNGTrainer/state.dat").exists());
        assert!(f.outside.join("Temp/a.tmp").exists());
    }

    #[test]
    fn a_fling_trainer_symlink_is_removed_as_a_link_its_target_survives() {
        let f = fixture();
        fs::create_dir_all(&f.local).unwrap();
        symlink(&f.outside, f.local.join("FLiNGTrainer")).unwrap();
        assert_eq!(TrainerDataCleanup::new(&f.prefix).run(false).unwrap(), 1);
        assert!(!exists(&f.local.join("FLiNGTrainer")));
        assert!(f.outside.join("precious.txt").exists());
    }

    #[test]
    fn a_prefix_without_the_folders_is_a_no_op() {
        let f = fixture();
        assert_eq!(TrainerDataCleanup::new(&f.prefix).run(false).unwrap(), 0);
        assert_eq!(
            TrainerDataCleanup::new(f.prefix.join("missing"))
                .run(false)
                .unwrap(),
            0
        );
    }

    #[test]
    fn targets_are_the_documented_fling_locations_of_the_proton_user() {
        assert_eq!(
            TrainerDataCleanup::new("/c/1").targets(),
            [
                PathBuf::from("/c/1/pfx/drive_c/users/steamuser/AppData/Local/FLiNGTrainer"),
                PathBuf::from("/c/1/pfx/drive_c/users/steamuser/AppData/Local/Temp"),
            ]
        );
    }
}
