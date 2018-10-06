use std::io;
use std::io::{ErrorKind as IOKind};
use std::cmp;
use std::fs::{File};
use std::path::{Path};
use std::ptr::null_mut;
use std::os::unix::io::AsRawFd;

use libc;

use crate::{Opts};
use crate::errors::{Result, XcpError};
use crate::utils::{to_err};



// Assumes Linux kernel > 4.5.
#[cfg(feature = "kernel_copy_file_range")]
unsafe fn copy_file_range(
    fd_in: libc::c_int,
    off_in: *mut libc::loff_t,
    fd_out: libc::c_int,
    off_out: *mut libc::loff_t,
    len: libc::size_t,
    flags: libc::c_uint,
) -> libc::ssize_t {
    libc::syscall(
        libc::SYS_copy_file_range,
        fd_in,
        off_in,
        fd_out,
        off_out,
        len,
        flags,
    ) as libc::ssize_t
}

// Requires GlibC >= 2.27
#[cfg(not(feature = "kernel_copy_file_range"))]
extern {
    fn copy_file_range(
        fd_in: libc::c_int,
        off_in: libc::loff_t,
        fd_out: libc::c_int,
        off_out: libc::loff_t,
        len: libc::size_t,
        flags: libc::c_uint,
    ) -> libc::ssize_t;
}

fn r_copy_file_range(infd: &File, outfd: &File, bytes: usize) -> Result<u64> {
    let r = unsafe {
        copy_file_range(
            infd.as_raw_fd(),
            null_mut(),
            outfd.as_raw_fd(),
            null_mut(),
            bytes, 0)
    };
    match r {
        -1 => Err(io::Error::last_os_error().into()),
        _ => Ok(r as u64)
    }
}


pub fn copy(from: &Path, to: &Path) -> Result<u64> {
    let infd = File::open(from)?;
    let outfd = File::create(to)?;
    let (perm, len) = {
        let metadata = infd.metadata()?;
        (metadata.permissions(), metadata.len())
    };

    let mut written = 0u64;
    while written < len {
        let bytes_to_copy = cmp::min(len - written, usize::max_value() as u64) as usize;
        let result = r_copy_file_range(&infd, &outfd, bytes_to_copy)?;
        written += result;
    }
    outfd.set_permissions(perm)?;
    Ok(written)
}



pub fn copy_single_file(opts: &Opts) -> Result<()> {
    let dest = if opts.dest.is_dir() {
        let fname = opts.source.file_name().ok_or(XcpError::UnknownFilename)?;
        opts.dest.join(fname)
    } else {
        opts.dest.clone()
    };

    if dest.is_file() && opts.noclobber {
        return Err(to_err(
            IOKind::AlreadyExists,
            "Destination file exists and no-clobber is set.",
        ));
    }

    copy(&opts.source, &dest)?;

    Ok(())
}
