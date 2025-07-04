// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use alloc::{ffi::CString, format, string::String, vec};
use blueos::{
    error::{
        code::{EEXIST, ENOENT, ENOTEMPTY},
        Error,
    },
    vfs::{
        dirent::{Dirent, DirentType},
        syscalls::*,
    },
};
use blueos_test_macro::test;
use core::{
    cmp::min,
    ffi::{c_char, c_int, CStr},
    fmt::Write,
};
use libc::{ENOSYS, O_CREAT, O_DIRECTORY, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, SEEK_SET};
use semihosting::println;

#[test]
fn vfs_test_uart() {
    // Test UART device path
    let uart_path = c"/dev/ttyS0";
    let path_ptr = uart_path.as_ptr() as *const c_char;

    let fd = vfs_open(path_ptr, O_RDWR, 0);
    assert!(fd >= 0, "[VFS Test devfs]: Failed to open UART device");

    let test_data = b"UART Test Message for a lot of data..................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................................end\n";
    let write_size = vfs_write(fd, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfs_close(fd);
        unreachable!(
            "[VFS Test devfs]: Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
    }

    let close_result = vfs_close(fd);
    assert!(
        close_result >= 0,
        "[VFS Test devfs]: Failed to close device"
    );
}

#[test]
fn vfs_test_read_and_write() {
    println!("[VFS Test Read/Write] Test the tmpfs mounted at /");
    test_read_and_write(String::from("/"), 30);
    test_read_and_write(String::from("/"), 2000);

    #[cfg(virtio)]
    {
        println!("[VFS Test Read/Write] Test the fatfs mounted at /fat");
        test_read_and_write(String::from("/fat/"), 30);
        test_read_and_write(String::from("/fat/"), 2_000);
        // Test: file size over a block size (default 4096 bytes)
        test_read_and_write(String::from("/fat/"), 6_000);
        test_read_and_write(String::from("/fat/"), 12_000);
        test_read_and_write(String::from("/fat/"), 24_000);
    }
}

fn test_read_and_write(path_prefix: String, test_data_len: usize) {
    // Test file path
    let mut test_path = path_prefix.clone();
    test_path.push_str("test.txt");
    let test_path = CString::new(test_path).expect("Failed to create CString");

    // Default file permissions: 644
    let mode: libc::mode_t = 0o644;

    let fd = vfs_open(test_path.as_ptr() as *const c_char, O_CREAT | O_RDWR, mode);
    assert!(fd >= 0, "[VFS Test Read/Write]  Failed to open file");

    let test_data = b"1".repeat(test_data_len);
    let write_size = vfs_write(fd, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfs_close(fd);
        unreachable!(
            "[VFS Test Read/Write]  Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
    }

    // Move file pointer back to start
    let seek_result = vfs_lseek(fd, 0, SEEK_SET);
    if seek_result < 0 {
        vfs_close(fd);
        unreachable!(
            "[VFS Test Read/Write]  Seek failed, error = {}",
            seek_result
        );
    }

    // Read data and verify
    let mut read_buf = vec![0u8; test_data.len()];
    let read_size = vfs_read(fd, read_buf.as_mut_ptr(), test_data.len());
    if read_size != test_data.len() as isize {
        vfs_close(fd);
        unreachable!(
            "[VFS Test Read/Write]  Read failed, expected {} bytes, read {}",
            test_data.len(),
            read_size
        );
    }

    // Verify read data
    let read_data = &read_buf[..min(test_data.len(), read_buf.len())];
    if read_data != test_data {
        vfs_close(fd);
        unreachable!(
            "[VFS Test Read/Write]  Data verification failed, Expected: {:?} Got: {:?}",
            test_data, read_data
        );
    }
    vfs_close(fd);

    // Open file with O_TRUNC
    let fd = vfs_open(test_path.as_ptr() as *const c_char, O_RDWR | O_TRUNC, mode);
    assert!(fd >= 0);
    vfs_close(fd);
}

#[test]
fn vfs_test_multiple_open() {
    println!("Test the tmpfs mounted at /");
    test_multiple_open(String::from("/"));

    #[cfg(virtio)]
    {
        println!("Test the fatfs mounted at /fat");
        test_multiple_open(String::from("/fat/"));
    }
}

fn test_multiple_open(path_prefix: String) {
    // Test file path
    let mut test_path = path_prefix.clone();
    test_path.push_str("test_multi.txt");
    let test_path = CString::new(test_path).expect("Failed to create CString");
    let path_ptr = test_path.as_ptr() as *const c_char;

    // First open for writing
    let fd1 = vfs_open(path_ptr, O_CREAT | O_RDWR, 0o644);
    assert!(fd1 >= 0, "[VFS Test MultiOpen]: Failed to open first fd");

    // Write test data
    let test_data = b"Hello, Multiple Open Test!\n";
    let write_size = vfs_write(fd1, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfs_close(fd1);
        unreachable!(
            "[VFS Test MultiOpen]: Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
    }

    // Second open for reading
    let fd2 = vfs_open(path_ptr, O_RDWR, 0o644);
    if fd2 < 0 {
        vfs_close(fd1);
        unreachable!(
            "[VFS Test MultiOpen]: Failed to open second fd, err = {}",
            fd2
        );
    }

    // Read data through second file descriptor
    let mut read_buf = [0u8; 64];
    let read_size = vfs_read(fd2, read_buf.as_mut_ptr(), test_data.len());
    if read_size != test_data.len() as isize {
        vfs_close(fd1);
        vfs_close(fd2);
        unreachable!(
            "[VFS Test MultiOpen]: Read failed, expected {} bytes, read {}",
            test_data.len(),
            read_size
        );
    }

    // Verify read data
    let read_data = &read_buf[..test_data.len()];
    if read_data != test_data {
        vfs_close(fd1);
        vfs_close(fd2);
        unreachable!(
            "[VFS Test MultiOpen]: Data verification failed, Expected: {:?} Got: {:?}",
            test_data, read_data
        );
    }

    // Close file descriptors
    vfs_close(fd1);
    vfs_close(fd2);
}

#[test]
fn vfs_test_directory_tree() {
    println!("[VFS Test DirctoryTree] Test the tmpfs mounted at /");
    test_directory_tree(String::from("/"));

    #[cfg(virtio)]
    {
        println!("[VFS Test DirctoryTree] Test the fatfs mounted at /fat");
        test_directory_tree(String::from("/fat/"));
    }
}

fn test_directory_tree(path_prefix: String) {
    // Create test directory structure:
    // /test_dir
    // /test_dir/dir1
    // /test_dir/dir2
    // /test_dir/dir1/subdir1
    // /test_dir/dir1/file1.txt

    // Create root test directory
    let mut root_dir = path_prefix.clone();
    root_dir.push_str("test_dir");
    let root_dir = CString::new(root_dir).expect("Failed to create CString");
    let result = vfs_mkdir(root_dir.as_ptr() as *const c_char, 0o755);
    assert!(
        result >= 0,
        "[VFS Test DirctoryTree]: Failed to create root test directory"
    );

    // Create subdirectory dir1
    let mut dir1 = path_prefix.clone();
    dir1.push_str("test_dir/dir1");
    let dir1 = CString::new(dir1).expect("Failed to create CString");
    let result = vfs_mkdir(dir1.as_ptr() as *const c_char, 0o755);
    if result < 0 {
        vfs_rmdir(root_dir.as_ptr() as *const c_char);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory dir1: {}",
            result
        );
    }

    // Create subdirectory dir2
    let mut dir2 = path_prefix.clone();
    dir2.push_str("test_dir/dir2");
    let dir2 = CString::new(dir2).expect("Failed to create CString");
    let result = vfs_mkdir(dir2.as_ptr() as *const c_char, 0o755);
    if result < 0 {
        vfs_rmdir(dir1.as_ptr() as *const c_char);
        vfs_rmdir(root_dir.as_ptr() as *const c_char);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory dir2: {}",
            result
        );
    }

    // Create subdirectory subdir1
    let mut subdir1 = path_prefix.clone();
    subdir1.push_str("test_dir/dir1/subdir1");
    let subdir1 = CString::new(subdir1).expect("Failed to create CString");
    let result = vfs_mkdir(subdir1.as_ptr() as *const c_char, 0o755);
    if result < 0 {
        vfs_rmdir(dir1.as_ptr() as *const c_char);
        vfs_rmdir(dir2.as_ptr() as *const c_char);
        vfs_rmdir(root_dir.as_ptr() as *const c_char);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory subdir1: {}",
            result
        );
    }

    // Create test file
    let mut file1 = path_prefix.clone();
    file1.push_str("test_dir/dir1/file1.txt");
    let file1 = CString::new(file1).expect("Failed to create CString");
    let fd = vfs_open(file1.as_ptr() as *const c_char, O_CREAT | O_RDWR, 0o755);
    if fd < 0 {
        vfs_rmdir(subdir1.as_ptr() as *const c_char);
        vfs_rmdir(dir1.as_ptr() as *const c_char);
        vfs_rmdir(dir2.as_ptr() as *const c_char);
        vfs_rmdir(root_dir.as_ptr() as *const c_char);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create test file: {}",
            fd
        );
    }
    vfs_close(fd);

    // Verify directory structure
    println!(
        "[VFS Test DirctoryTree] Verify {}",
        root_dir.to_str().unwrap()
    );
    match verify_directory(root_dir.as_ptr() as *const c_char) {
        Ok(_) => {}
        Err(err) => {
            unreachable!(
                "[VFS Test DirctoryTree]:  Verification failed with error {}",
                err
            );
        }
    }

    // Delete 1 file and 2 dirs, the final directory structure should be:
    // /test_dir
    // /test_dir/dir2

    // Delete file /test_dir/dir1/file1.txt
    assert_eq!(vfs_unlink(file1.as_ptr() as *const c_char), 0);

    // Delete directory /test_dir/dir1, expected to fail because the directory is not empty
    assert_eq!(
        vfs_rmdir(dir1.as_ptr() as *const c_char),
        ENOTEMPTY.to_errno()
    );

    // Delete directory /test_dir/dir1/file1.txt
    assert_eq!(vfs_rmdir(subdir1.as_ptr() as *const c_char), 0);

    // Delete directory /test_dir/dir1
    assert_eq!(vfs_rmdir(dir1.as_ptr() as *const c_char), 0);

    // Verify the deleted directory
    let fd = vfs_open(subdir1.as_ptr() as *const c_char, O_RDWR, 0o755);
    assert!(fd == ENOENT.to_errno());
    vfs_close(fd);

    // Verify directory structure
    println!(
        "[VFS Test DirctoryTree] Verify {}",
        root_dir.to_str().unwrap()
    );
    match verify_directory(root_dir.as_ptr() as *const c_char) {
        Ok(_) => {}
        Err(err) => {
            unreachable!(
                "[VFS Test DirctoryTree]:  Verification failed with error {}",
                err
            );
        }
    }
}

fn verify_directory(path: *const c_char) -> Result<(), c_int> {
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let dir = vfs_open(path, O_RDONLY, 0o755);
    if dir < 0 {
        println!("[VFS Test DirctoryTree] open fail,  fd: {}", dir);
        return Err(-ENOSYS);
    };
    let mut buf = [0u8; 256];
    // Print return value of each readdir call
    let len = vfs_getdents(dir, buf.as_mut_ptr(), buf.len());
    if len < 0 {
        vfs_close(dir);
        return Err(len);
    }
    let mut next_entry = 0;
    while next_entry < len as usize {
        let entry = unsafe { Dirent::from_buf_ref(&buf[next_entry..]) };
        let name = entry.name().unwrap().to_string_lossy();
        let mut dir_full_path = String::with_capacity(name.len() + 1 + path_str.len());
        let _ = write!(dir_full_path, "{}/{}", path_str, name);
        if entry.type_() == DirentType::Dir {
            println!(
                "\t[VFS Test DirctoryTree]: Found directory: {}, {}, {}",
                entry.ino(),
                entry.off(),
                dir_full_path
            );
            if name.as_ref() != "." && name.as_ref() != ".." {
                verify_directory(
                    CString::new(dir_full_path.as_str())
                        .expect("Invalid string")
                        .as_ptr(),
                )?;
            }
        } else {
            println!(
                "\t[VFS Test DirctoryTree]: Found file     : {}, {}, {}",
                entry.ino(),
                entry.off(),
                dir_full_path
            );
        }
        next_entry += entry.reclen() as usize;
    }
    // Close directory
    vfs_close(dir);
    Ok(())
}

#[test]
fn vfs_test_std_fds() {
    // Test writing to stdout (fd 1)
    let test_data = b"Hello, this is a test message to stdout!\n";
    let write_size = vfs_write(1, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        println!(
            "[VFS Test STD FDs]: Write to stdout failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
        unreachable!();
    }

    // Test writing to stderr (fd 2)
    let error_data = b"This is an error message to stderr!\n";
    let write_size = vfs_write(2, error_data.as_ptr(), error_data.len());
    if write_size != error_data.len() as isize {
        println!(
            "[VFS Test STD FDs]: Write to stderr failed, expected {} bytes, wrote {}",
            error_data.len(),
            write_size
        );
        unreachable!();
    }
}

#[cfg(virtio)]
#[test]
fn vfs_test_fatfs_mount_unmount() {
    let mode: libc::mode_t = 0o644;
    let mount_path_1 = c"/fat".as_ptr() as *const c_char;
    let mount_path_2 = c"/fat2".as_ptr() as *const c_char;

    // Unmount /fat
    assert_eq!(vfs_umount(mount_path_1), 0);

    // Mount the fatfs using the virt-storage device to /fat2
    assert!(vfs_mkdir(mount_path_2, mode) == 0);
    assert_eq!(
        vfs_mount(
            c"virt-storage".as_ptr() as *const c_char,
            mount_path_2,
            c"fatfs".as_ptr() as *const c_char,
            0,
            core::ptr::null(),
        ),
        0
    );

    // Create a file and write something
    let fd = vfs_open(
        c"/fat2/test.txt".as_ptr() as *const c_char,
        O_CREAT | O_RDWR,
        mode,
    );
    assert!(fd >= 0);
    let test_data = b"Hello, BlueKernel!\n";
    let write_size = vfs_write(fd, test_data.as_ptr(), test_data.len());
    assert!(write_size == test_data.len() as isize);
    vfs_close(fd);

    // Unmount /fat2
    assert_eq!(vfs_umount(mount_path_2), 0);

    // Trying to create the directory /fat, expected failure because the path exists
    assert_eq!(vfs_mkdir(mount_path_1, mode), EEXIST.to_errno());
    // Mount the fatfs using the virt-storage device to /fat
    assert_eq!(
        vfs_mount(
            c"virt-storage".as_ptr() as *const c_char,
            mount_path_1,
            c"fatfs".as_ptr() as *const c_char,
            0,
            core::ptr::null(),
        ),
        0
    );

    // Read the file and check content
    let fd = vfs_open(c"/fat/test.txt".as_ptr() as *const c_char, O_RDONLY, mode);
    assert!(fd >= 0);
    let mut read_buf = [0u8; 64];
    let read_size = vfs_read(fd, read_buf.as_mut_ptr(), test_data.len());
    assert!(read_size == test_data.len() as isize);
    // Verify read data
    let read_data = &read_buf[..test_data.len()];
    assert!(read_data == test_data);
    vfs_close(fd);
}

#[cfg(procfs)]
#[test]
fn vfs_test_procfs_posix() {
    // 1. Test: read /proc/meminfo
    let path = c"/proc/meminfo".as_ptr() as *const c_char;
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let fd = vfs_open(path, O_WRONLY, 0o444);
    assert!(
        fd == -libc::EACCES,
        "[VFS Test proc posix] The open operation should fail due to incorrect permissions"
    );

    let fd = vfs_open(path, O_RDONLY, 0o444);
    assert!(
        fd >= 0,
        "[VFS Test proc posix]  Failed to open file {}",
        path_str
    );
    let read_size = read_fd_content(path_str, fd);
    assert!(
        read_size > 0,
        "[VFS Test proc posix] Failed to read {}",
        path_str
    );
    vfs_close(fd);

    // 2. Test: read /proc/stat
    let path = c"/proc/stat".as_ptr() as *const c_char;
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let fd = vfs_open(path, O_RDONLY, 0o444);
    assert!(
        fd >= 0,
        "[VFS Test proc posix]  Failed to open file {}",
        path_str
    );
    let read_size = read_fd_content(path_str, fd);
    assert!(
        read_size > 0,
        "[VFS Test proc posix] Failed to read {}",
        path_str
    );
    vfs_close(fd);

    // 3. Test: readdir /proc & read /proc/{tid}/task
    let path = c"/proc".as_ptr() as *const c_char;
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let fd = vfs_open(path, O_RDONLY, 0o555);
    if fd < 0 {
        unreachable!("[VFS Test proc posix]: Failed to open file {}", path_str);
    }
    let mut buf = [0u8; 1024];
    let len = vfs_getdents(fd, buf.as_mut_ptr(), buf.len());
    if len < 0 {
        vfs_close(fd);
        unreachable!("[VFS Test proc posix]: Failed to getdents {}", path_str);
    }
    let mut next_entry = 0;
    while next_entry < len as usize {
        let entry = unsafe { Dirent::from_buf_ref(&buf[next_entry..]) };
        let name = entry.name().unwrap().to_string_lossy();
        let mut dir_full_path = String::with_capacity(name.len() + 1 + path_str.len());
        write!(dir_full_path, "{}/{}", path_str, name);
        if entry.type_() == DirentType::Dir && name.as_ref() != "." && name.as_ref() != ".." {
            let status_path = format!("{}/status\0", dir_full_path);
            let status_path_str = status_path.as_ptr() as *const c_char;
            let fd = vfs_open(status_path_str, O_RDONLY, 0o444);
            assert!(
                fd >= 0,
                "Failed to open file {}, error = {}",
                status_path,
                fd
            );
            let read_size = read_fd_content(status_path.as_str(), fd);
            assert!(
                read_size > 0,
                "[VFS Test proc posix] Failed to read {}",
                status_path
            );
            vfs_close(fd);
        }
        next_entry += entry.reclen() as usize;
    }
    vfs_close(fd);
}

fn read_fd_content(path_str: &str, fd: i32) -> usize {
    let mut read_buf;
    let mut read_size = 0;
    let mut result = String::new();
    loop {
        read_buf = [0u8; 64];
        let tmp_size = vfs_read(fd, read_buf.as_mut_ptr(), read_buf.len());
        if tmp_size < 0 {
            unreachable!(
                "[VFS Test proc posix]: Failed to read {}, error = {}",
                path_str, tmp_size
            );
        }
        let tmp: alloc::borrow::Cow<'_, str> =
            String::from_utf8_lossy(&read_buf[..tmp_size as usize]);
        result.push_str(tmp.as_ref());
        read_size += tmp_size;
        if tmp_size == 0 {
            break;
        }
    }
    println!("[VFS Test] read {} content:\n{}", path_str, result);
    read_size as usize
}
