// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::path::{self, Path, PathBuf};
use std::ffi::OsString;
use std::fs;
use std::io;

// Unfortunately, on windows, it looks like msvcrt.dll is silently translating
// verbatim paths under the hood to non-verbatim paths! This manifests itself as
// gcc looking like it cannot accept paths of the form `\\?\C:\...`, but the
// real bug seems to lie in msvcrt.dll.
//
// Verbatim paths are generally pretty rare, but the implementation of
// `fs::canonicalize` currently generates paths of this form, meaning that we're
// going to be passing quite a few of these down to gcc, so we need to deal with
// this case.
//
// For now we just strip the "verbatim prefix" of `\\?\` from the path. This
// will probably lose information in some cases, but there's not a whole lot
// more we can do with a buggy msvcrt...
//
// For some more information, see this comment:
//   https://github.com/rust-lang/rust/issues/25505#issuecomment-102876737
pub fn fix_windows_verbatim_for_gcc(p: &Path) -> PathBuf {
    if !cfg!(windows) {
        return p.to_path_buf();
    }
    let mut components = p.components();
    let prefix = match components.next() {
        Some(path::Component::Prefix(p)) => p,
        _ => return p.to_path_buf(),
    };
    match prefix.kind() {
        path::Prefix::VerbatimDisk(disk) => {
            let mut base = OsString::from(format!("{}:", disk as char));
            base.push(components.as_path());
            PathBuf::from(base)
        }
        path::Prefix::VerbatimUNC(server, share) => {
            let mut base = OsString::from(r"\\");
            base.push(server);
            base.push(r"\");
            base.push(share);
            base.push(components.as_path());
            PathBuf::from(base)
        }
        _ => p.to_path_buf(),
    }
}

pub enum LinkOrCopy {
    Link,
    Copy,
}

/// Copy `p` into `q`, preferring to use hard-linking if possible. If
/// `q` already exists, it is removed first.
/// The result indicates which of the two operations has been performed.
pub fn link_or_copy<P: AsRef<Path>, Q: AsRef<Path>>(p: P, q: Q) -> io::Result<LinkOrCopy> {
    let p = p.as_ref();
    let q = q.as_ref();
    if q.exists() {
        fs::remove_file(&q)?;
    }

    match fs::hard_link(p, q) {
        Ok(()) => Ok(LinkOrCopy::Link),
        Err(_) => {
            match fs::copy(p, q) {
                Ok(_) => Ok(LinkOrCopy::Copy),
                Err(e) => Err(e),
            }
        }
    }
}

#[derive(Debug)]
pub enum RenameOrCopyRemove {
    Rename,
    CopyRemove,
}

/// Rename `p` into `q`, preferring to use `rename` if possible.
/// If `rename` fails (rename may fail for reasons such as crossing
/// filesystem), fallback to copy & remove
pub fn rename_or_copy_remove<P: AsRef<Path>, Q: AsRef<Path>>(p: P,
                                                             q: Q)
                                                             -> io::Result<RenameOrCopyRemove> {
    let p = p.as_ref();
    let q = q.as_ref();
    match fs::rename(p, q) {
        Ok(()) => Ok(RenameOrCopyRemove::Rename),
        Err(_) => {
            match fs::copy(p, q) {
                Ok(_) => {
                    fs::remove_file(p)?;
                    Ok(RenameOrCopyRemove::CopyRemove)
                }
                Err(e) => Err(e),
            }
        }
    }
}
