use crate::{
    c_str::CStr,
    errno::{Errno, ERRNO},
    stdio::File,
    stdlib::malloc::{free, malloc},
    string::memcpy,
    syscall::{Sys, Syscall},
};

use alloc::{boxed::Box, vec::Vec};
#[allow(unused_imports)]
use bluekernel_header::syscalls::NR::{Close, Lseek, Mount, Umount};
use bluekernel_scal::bk_syscall;
#[allow(unused_imports)]
use core::{ptr, slice};
use libc::{c_char, c_int, c_long, c_ulong, c_void, off_t, EINVAL, EIO, ENOMEM, SEEK_SET};

const INITIAL_BUFSIZE: usize = 512;
pub struct DIR {
    pub file: File,
    pub name: *const c_char,
    pub buf: Vec<u8>,
    pub buf_offset: usize,
    pub opaque_offset: usize,
}

impl DIR {
    pub fn new(path: CStr) -> Result<Box<Self>, Errno> {
        Ok(Box::new(Self {
            file: File::open(
                path.clone(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )?,
            name: path.as_ptr(),
            buf: Vec::with_capacity(INITIAL_BUFSIZE),
            buf_offset: 0,
            opaque_offset: 0,
        }))
    }
    fn next_dirent(&mut self) -> Result<*mut Dirent, Errno> {
        let mut this_dent = self.buf.get(self.buf_offset..).ok_or(Errno(EIO))?;
        if this_dent.is_empty() {
            let size = loop {
                self.buf.resize(self.buf.capacity(), 0_u8);

                match Sys::getdents(*self.file, &mut self.buf) {
                    Ok(size) => break size,
                    Err(Errno(EINVAL)) => {
                        self.buf
                            .try_reserve_exact(self.buf.len())
                            .map_err(|_| Errno(ENOMEM))?;
                        continue;
                    }
                    Err(Errno(other)) => return Err(Errno(other)),
                }
            };

            self.buf.truncate(size);
            self.buf_offset = 0;

            if size == 0 {
                return Ok(core::ptr::null_mut());
            }
            this_dent = &self.buf;
        }
        let (this_reclen, this_next_opaque) = unsafe {
            let dent = this_dent.as_ptr() as *const Dirent;
            ((*dent).d_reclen as usize, (*dent).d_off as usize)
        };
        let next_off = self.buf_offset.checked_add(this_reclen).ok_or(Errno(EIO))?;
        if next_off > self.buf.len() {
            return Err(Errno(EIO));
        }

        if this_dent.len() < usize::from(this_reclen) {
            return Err(Errno(EIO));
        }
        let dent_ptr = this_dent.as_ptr() as *mut Dirent;

        self.opaque_offset = this_next_opaque;
        self.buf_offset = next_off;
        Ok(dent_ptr)
    }
    fn seek(&mut self, off: u64) {
        let ret = bk_syscall!(Lseek, *self.file, off as usize, SEEK_SET) as off_t;
        if ret < 0 {
            return;
        }
        self.buf.clear();
        self.buf_offset = 0;
        self.opaque_offset = off as usize;
    }
    fn rewind(&mut self) {
        self.opaque_offset = 0;
        bk_syscall!(Lseek, *self.file, 0, SEEK_SET);
        self.buf.clear();
        self.buf_offset = 0;
        self.opaque_offset = 0;
    }
    fn close(mut self) -> Result<(), Errno> {
        self.file.reference = true;
        let err = bk_syscall!(Close, *self.file) as c_int;
        if err < 0 {
            return Err(Errno(err));
        }
        Ok(())
    }
}

/// copy from kernel/kernel/src/vfs/dirent.rs
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Dirent {
    d_ino: usize,
    d_off: usize,
    /// The length of the dirent
    d_reclen: u16,
    /// The type of the file
    d_type: u8,
    // The file name - flexible array member
    d_name: [u8; 0],
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/closedir.html>.
#[no_mangle]
pub extern "C" fn closedir(dir: Box<DIR>) -> c_int {
    dir.close().map(|_| 0).unwrap_or(-1)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fdopendir.html>.
#[no_mangle]
pub unsafe extern "C" fn opendir(path: *const c_char) -> *mut DIR {
    let path = unsafe { CStr::from_ptr(path) };

    let od = DIR::new(path);
    match od {
        Ok(dir) => Box::into_raw(dir) as *mut DIR,
        Err(Errno(errno)) => {
            ERRNO.set(errno);
            core::ptr::null_mut()
        }
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/readdir.html>.
#[no_mangle]
pub extern "C" fn readdir(dir: &mut DIR) -> *mut Dirent {
    let nd = dir.next_dirent();
    match nd {
        Ok(ptr) => ptr,
        Err(Errno(errno)) => {
            ERRNO.set(errno);
            core::ptr::null_mut()
        }
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/readdir.html>.
///
/// # Deprecation
/// The `readdir_r()` function was marked obsolescent in the Open Group Base
/// Specifications Issue 8.
#[deprecated]
// #[no_mangle]
pub extern "C" fn readdir_r(
    _dir: *mut DIR,
    _entry: *mut Dirent,
    _result: *mut *mut Dirent,
) -> *mut Dirent {
    unimplemented!(); // plus, deprecated
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/rewinddir.html>.
#[no_mangle]
pub extern "C" fn rewinddir(dir: &mut DIR) {
    dir.rewind();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/scandir.html>.
#[no_mangle]
pub unsafe extern "C" fn scandir(
    dirp: *const c_char,
    namelist: *mut *mut *mut Dirent,
    filter: Option<extern "C" fn(_: *const Dirent) -> c_int>,
    _compare: Option<extern "C" fn(_: *mut *const Dirent, _: *mut *const Dirent) -> c_int>,
) -> c_int {
    let dir = unsafe { opendir(dirp) };
    if dir.is_null() {
        return -1;
    }

    let mut vec = Vec::with_capacity(4);

    let old_errno = ERRNO.get();
    ERRNO.set(0);

    loop {
        let entry: *mut Dirent = readdir(unsafe { &mut *dir });
        if entry.is_null() {
            break;
        }

        if let Some(filter) = filter {
            if filter(entry) == 0 {
                continue;
            }
        }

        let copy = unsafe { malloc((*entry).d_reclen as usize) };
        if copy.is_null() {
            break;
        }
        unsafe {
            memcpy(
                copy as *mut core::ffi::c_void,
                entry as *const core::ffi::c_void,
                (*entry).d_reclen as usize,
            )
        };
        vec.push(copy);
    }

    closedir(unsafe { Box::from_raw(dir) });

    let len = vec.len();
    vec.shrink_to_fit();

    if ERRNO.get() != 0 {
        for ptr in &mut vec {
            unsafe { free(*ptr as *mut c_void) };
        }
        -1
    } else {
        unsafe {
            *namelist = vec.leak().as_mut_ptr() as *mut *mut Dirent;
        }

        ERRNO.set(old_errno);
        // todo: sort?
        len as c_int
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/seekdir.html>.
#[no_mangle]
pub extern "C" fn seekdir(dir: &mut DIR, off: c_long) {
    dir.seek(
        off.try_into()
            .expect("off must come from telldir, thus never negative"),
    );
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/telldir.html>.
#[no_mangle]
pub extern "C" fn telldir(dir: &mut DIR) -> c_long {
    dir.opaque_offset as c_long
}
#[no_mangle]
pub unsafe extern "C" fn mount(
    _source: *const c_char,
    _target: *const c_char,
    _filesystemtype: *const c_char,
    _mountflags: c_ulong,
    _data: *const c_void,
) -> c_int {
    // NOTE: not posix call
    bk_syscall!(Mount, _source, _target, _filesystemtype, _mountflags, _data) as c_int
}

#[no_mangle]
pub unsafe extern "C" fn umount(_target: *const c_char) -> c_int {
    // NOTE: not posix call
    bk_syscall!(Umount, _target) as c_int
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::println;
    use bluekernel_test_macro::test;

    #[test]
    fn test_scandir() {
        let path = CStr::from_bytes_with_nul(b"/\0").unwrap();
        let mut namelist: *mut *mut Dirent = ptr::null_mut();
        let count = unsafe { scandir(path.as_ptr(), &mut namelist, None, None) };
        assert!(count >= 0);
        if count > 0 {
            unsafe {
                for i in 0..count as usize {
                    let entry = *namelist.add(i);
                    println!(
                        "Entry {}: {}",
                        i,
                        CStr::from_ptr((*entry).d_name.as_ptr() as *const c_char)
                            .to_str()
                            .unwrap()
                    );
                    free(entry as *mut c_void);
                }
                free(namelist as *mut c_void);
            }
        }
    }
}
