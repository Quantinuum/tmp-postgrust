use std::path::{Path, PathBuf};

use glob::glob;
use which::which;

use crate::errors::{TmpPostgrustError, TmpPostgrustResult};

/// Additional file system locations to search for binaries
/// if `initdb` and `postgres` are not in the $PATH.
const SEARCH_PATHS: [&str; 5] = [
    "/usr/local/pgsql",
    "/usr/local",
    "/usr/pgsql-*",
    "/usr/lib/postgresql/*",
    "/opt/local/lib/postgresql*",
];

/// Locate a postgres binary by name.
///
/// If `bin_dir` is `Some`, only that directory is searched, with no `$PATH` fallback.
/// If `bin_dir` is `None`, `$PATH` is tried first, then well-known install locations are tried as a fallback.
pub(crate) fn find_postgresql_command(
    bin_dir: Option<&Path>,
    name: &str,
) -> TmpPostgrustResult<PathBuf> {
    if let Some(dir) = bin_dir {
        // N.B. effectively this "." parameter doesn't do anything since we
        // never pass in any relative paths, only command names.
        return which::which_in(name, Some(dir), std::path::Path::new(".")).map_err(|_| {
            TmpPostgrustError::PostgresCommandNotFound {
                command: name.to_string(),
                searched_dir: Some(dir.to_owned()),
            }
        });
    }

    // Use binaries from $PATH if available.
    if let Ok(path) = which(name) {
        return Ok(path);
    }

    // Check common install locations for the first available postgresql.
    for path in SEARCH_PATHS {
        if let Some(entry) = glob(&(path.to_string() + "/bin/" + name))
            .expect("Failed to read glob pattern")
            .flatten()
            .next()
        {
            return Ok(entry);
        }
    }

    Err(TmpPostgrustError::PostgresCommandNotFound {
        command: name.to_string(),
        searched_dir: None,
    })
}

/// Resolved absolute paths for all postgres binaries the library needs.
#[derive(Debug)]
pub(crate) struct PostgresBinaries {
    pub postgres: PathBuf,
    pub initdb: PathBuf,
    pub createdb: PathBuf,
    pub createuser: PathBuf,
}

impl PostgresBinaries {
    pub(crate) fn resolve(bin_dir: Option<&Path>) -> TmpPostgrustResult<Self> {
        Ok(Self {
            postgres: find_postgresql_command(bin_dir, "postgres")?,
            initdb: find_postgresql_command(bin_dir, "initdb")?,
            createdb: find_postgresql_command(bin_dir, "createdb")?,
            createuser: find_postgresql_command(bin_dir, "createuser")?,
        })
    }
}

/// Return a tuple of directory paths and other paths in a sub path by recursing through
/// and reading all directories.
pub(crate) fn all_dir_entries(src_dir: &Path) -> TmpPostgrustResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut dirs = Vec::new();
    let mut others = Vec::new();
    for read_dir in src_dir
        .read_dir()
        .map_err(TmpPostgrustError::CopyCachedInitDBFailedFileNotFound)?
    {
        let entry = read_dir.map_err(TmpPostgrustError::CopyCachedInitDBFailedFileNotFound)?;
        let entry_file_type = entry
            .file_type()
            .map_err(TmpPostgrustError::CopyCachedInitDBFailedCouldNotReadFileType)?;

        if entry_file_type.is_dir() {
            let (sub_dirs, sub_others) = all_dir_entries(&entry.path())?;
            dirs.push(entry.path());
            dirs.extend(sub_dirs);
            others.extend(sub_others);
        } else {
            others.push(entry.path());
        }
    }
    Ok((dirs, others))
}

pub(crate) fn build_copy_dst_path(
    target_path: &Path,
    src_dir: &Path,
    dst_dir: &Path,
) -> TmpPostgrustResult<PathBuf> {
    let entry_sub_path = target_path
        .strip_prefix(src_dir)
        .map_err(TmpPostgrustError::CopyCachedInitDBFailedCouldNotStripPathPrefix)?;
    let dst_path = dst_dir.join(entry_sub_path);

    Ok(dst_path)
}
