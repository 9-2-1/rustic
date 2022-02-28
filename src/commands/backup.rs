use std::fs::{read_link, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{TimeZone, Utc};
use clap::Parser;
use ignore::{DirEntry, WalkBuilder};
use path_absolutize::*;

use crate::archiver::Archiver;
use crate::backend::DecryptFullBackend;
use crate::blob::{Metadata, Node};
use crate::index::IndexBackend;
use crate::repo::ConfigFile;

#[derive(Parser)]
pub(super) struct Opts {
    /// save access time for files and directories
    #[clap(long)]
    with_atime: bool,

    /// backup sources
    sources: Vec<String>,
}

pub(super) fn execute(opts: Opts, be: &impl DecryptFullBackend) -> Result<()> {
    let config = ConfigFile::from_backend_no_id(be)?;

    let poly = u64::from_str_radix(config.chunker_polynomial(), 16)?;
    let path = PathBuf::from(&opts.sources[0]);
    let path = path.absolutize()?;
    backup(path.into(), &poly, be, opts)?;
    Ok(())
}

fn backup(
    backup_path: PathBuf,
    poly: &u64,
    be: &impl DecryptFullBackend,
    opts: Opts,
) -> Result<()> {
    println! {"reading index..."}
    let index = IndexBackend::new(be)?;
    let mut archiver = Archiver::new(be.clone(), index, *poly)?;

    let mut wb = WalkBuilder::new(backup_path.clone());
    /*
     for path in paths[1..].into_iter() {
        wb.add(path);
    }
    */
    wb.follow_links(false).hidden(false);

    let nodes = wb.build().map(|entry| map_entry(entry?, opts.with_atime));

    for res in nodes {
        let (path, node, r) = res?;
        archiver.add_entry(&path, node, r)?;
    }
    archiver.finalize_snapshot(backup_path)?;

    Ok(())
}

fn map_entry(entry: DirEntry, with_atime: bool) -> Result<(PathBuf, Node, Option<impl BufRead>)> {
    let name = entry.file_name().to_os_string();
    let m = entry.metadata()?;

    let meta = Metadata::new(
        m.len(),
        m.modified().ok().map(|t| t.into()),
        if with_atime {
            m.accessed().ok().map(|t| t.into())
        } else {
            // TODO: Use None here?
            m.modified().ok().map(|t| t.into())
        },
        Some(Utc.timestamp(m.ctime(), m.ctime_nsec().try_into()?).into()),
        m.mode(),
        m.uid(),
        m.gid(),
        "".to_string(),
        "".to_string(),
        m.ino(),
        m.dev(),
        m.nlink(),
    );
    let (node, r) = if m.is_dir() {
        (Node::new_dir(name, meta), None)
    } else if m.is_symlink() {
        let target = read_link(entry.path())?;
        (Node::new_symlink(name, target, meta), None)
    } else {
        let f = File::open(&entry.path())?;
        (Node::new_file(name, meta), Some(BufReader::new(f)))
    };
    Ok((entry.path().to_path_buf(), node, r))
}