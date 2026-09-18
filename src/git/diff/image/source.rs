#[cfg(test)]
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use super::ImageSide;
use super::decode::MAX_IMAGE_BYTES;
use crate::git::read_git_blob;
use crate::git::support::safe_worktree_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlobOrigin<'a> {
    Missing,
    Revision(&'a str),
    Index,
    Worktree,
}

#[derive(Debug, Clone)]
pub(crate) enum LoadedBlob {
    Missing,
    TooLarge { size: usize },
    Bytes(Vec<u8>),
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct MapImageSource {
    pub previous: HashMap<PathBuf, Vec<u8>>,
    pub current: HashMap<PathBuf, Vec<u8>>,
}

pub(crate) trait ImageBlobSource {
    fn load(&self, path: &Path, old_path: Option<&Path>, side: ImageSide) -> LoadedBlob;
}

#[cfg(test)]
impl ImageBlobSource for MapImageSource {
    fn load(&self, path: &Path, old_path: Option<&Path>, side: ImageSide) -> LoadedBlob {
        let key = match side {
            ImageSide::Previous => old_path.unwrap_or(path),
            ImageSide::New => path,
        };
        let source = match side {
            ImageSide::Previous => &self.previous,
            ImageSide::New => &self.current,
        };
        source
            .get(key)
            .cloned()
            .map_or(LoadedBlob::Missing, |bytes| {
                if bytes.len() > MAX_IMAGE_BYTES {
                    LoadedBlob::TooLarge { size: bytes.len() }
                } else {
                    LoadedBlob::Bytes(bytes)
                }
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RevisionImageSource<'a> {
    pub git_dir: &'a Path,
    pub worktree: &'a Path,
    pub previous: BlobOrigin<'a>,
    pub current: BlobOrigin<'a>,
}

impl ImageBlobSource for RevisionImageSource<'_> {
    fn load(&self, path: &Path, old_path: Option<&Path>, side: ImageSide) -> LoadedBlob {
        let origin = match side {
            ImageSide::Previous => self.previous,
            ImageSide::New => self.current,
        };
        let blob_path = match side {
            ImageSide::Previous => old_path.unwrap_or(path),
            ImageSide::New => path,
        };
        load_origin(self.git_dir, self.worktree, origin, blob_path)
    }
}

fn load_origin(git_dir: &Path, worktree: &Path, origin: BlobOrigin<'_>, path: &Path) -> LoadedBlob {
    match origin {
        BlobOrigin::Missing => LoadedBlob::Missing,
        BlobOrigin::Revision(revision) => {
            read_git_blob(git_dir, &blob_spec(revision, path), MAX_IMAGE_BYTES)
        }
        BlobOrigin::Index => {
            let spec = format!(":{}", path_spec(path));
            read_git_blob(git_dir, &spec, MAX_IMAGE_BYTES)
        }
        BlobOrigin::Worktree => read_worktree_blob(worktree, path),
    }
}

fn blob_spec(revision: &str, path: &Path) -> String {
    let mut spec = String::new();
    spec.push_str(revision);
    spec.push(':');
    spec.push_str(&path_spec(path));
    spec
}

fn path_spec(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_worktree_blob(root: &Path, path: &Path) -> LoadedBlob {
    let Ok(absolute) = safe_worktree_path(root, path) else {
        return LoadedBlob::Missing;
    };
    let Ok(metadata) = std::fs::symlink_metadata(&absolute) else {
        return LoadedBlob::Missing;
    };
    if !metadata.is_file() {
        return LoadedBlob::Missing;
    }
    let size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    if size > MAX_IMAGE_BYTES {
        return LoadedBlob::TooLarge { size };
    }
    let Ok(file) = std::fs::File::open(&absolute) else {
        return LoadedBlob::Missing;
    };
    let mut bytes = Vec::new();
    match file
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        Ok(_) if bytes.len() > MAX_IMAGE_BYTES => LoadedBlob::TooLarge {
            size: size.max(bytes.len()),
        },
        Ok(_) => LoadedBlob::Bytes(bytes),
        Err(_) => LoadedBlob::Missing,
    }
}
