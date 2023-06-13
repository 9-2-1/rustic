use std::path::Path;

use bytes::Bytes;

use crate::{
    error::CommandErrorKind, repository::IndexedRepository, BlobType, DecryptReadBackend, FileType,
    Id, IndexedBackend, OpenRepository, ProgressBars, ReadBackend, RusticResult, SnapshotFile,
    Tree,
};

pub fn cat_file(repo: &OpenRepository, tpe: FileType, id: &str) -> RusticResult<Bytes> {
    let id = repo.dbe.find_id(tpe, id)?;
    let data = repo.dbe.read_encrypted_full(tpe, &id)?;
    Ok(data)
}

pub fn cat_blob(repo: &IndexedRepository, tpe: BlobType, id: &str) -> RusticResult<Bytes> {
    let id = Id::from_hex(id)?;
    let data = repo.index.blob_from_backend(tpe, &id)?;

    Ok(data)
}

pub fn cat_tree(
    repo: &IndexedRepository,
    snap: &str,
    sn_filter: impl FnMut(&SnapshotFile) -> bool + Send + Sync,
    pb: &impl ProgressBars,
) -> RusticResult<Bytes> {
    let (id, path) = snap.split_once(':').unwrap_or((snap, ""));
    let snap = SnapshotFile::from_str(
        &repo.repo.dbe,
        id,
        sn_filter,
        &pb.progress_counter("getting snapshot..."),
    )?;
    let node = Tree::node_from_path(&repo.index, snap.tree, Path::new(path))?;
    let id = node
        .subtree
        .ok_or_else(|| CommandErrorKind::PathIsNoDir(path.to_string()))?;
    let data = repo.index.blob_from_backend(BlobType::Tree, &id)?;
    Ok(data)
}
