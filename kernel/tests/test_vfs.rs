#![allow(dead_code)]

use alloc::string::String;
#[cfg(procfs)]
use bluekernel::{error::code, libc::O_WRONLY, vfs::procfs::ProcFileSystem};
use bluekernel::{
    println,
    vfs::{
        dirent::{Dirent, DirentType},
        posix::*,
    },
};
use bluekernel_test_macro::test;
use core::ffi::{c_char, c_int};
use libc::{mode_t, ENOSYS, O_CREAT, O_RDONLY, O_RDWR, SEEK_SET};

#[test]
fn vfs_test_uart() {
    // Test UART device path
    let uart_path = b"/dev/ttyS0\0";
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
    // Test file path
    let test_path = b"/test.txt\0";
    let path_ptr = test_path.as_ptr() as *const c_char;

    // Open flags: create and read/write
    let flags = O_CREAT | O_RDWR;

    // Default file permissions: 644
    let mode: mode_t = 0o644;

    let fd = vfs_open(path_ptr, flags, mode);
    assert!(fd >= 0, "[VFS Test Read/Write]  Failed to open file");

    let test_data = b"Hello, Blue Kernel!\n";
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
    let mut read_buf = [0u8; 64];
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
    let read_data = &read_buf[..test_data.len()];
    if read_data != test_data {
        vfs_close(fd);
        unreachable!(
            "[VFS Test Read/Write]  Data verification failed, Expected: {:?} Got: {:?}",
            test_data, read_data
        );
    }

    vfs_close(fd);
}

#[test]
fn vfs_test_multiple_open() {
    // Test file path
    let test_path = b"/test_multi.txt\0";
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
    // Create test directory structure:
    // /test_dir
    // /test_dir/dir1
    // /test_dir/dir2
    // /test_dir/dir1/subdir1
    // /test_dir/dir1/file1.txt

    // Create root test directory
    let root_dir = b"/test_dir\0";
    let result = vfs_mkdir(root_dir.as_ptr() as *const c_char, 0o755);
    assert!(
        result >= 0,
        "[VFS Test DirctoryTree]: Failed to create root test directory"
    );

    // Create subdirectory dir1
    let dir1 = b"/test_dir/dir1\0";
    let result = vfs_mkdir(dir1.as_ptr() as *const c_char, 0o755);
    if result < 0 {
        vfs_rmdir(root_dir.as_ptr() as *const c_char);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory dir1: {}",
            result
        );
    }

    // Create subdirectory dir2
    let dir2 = b"/test_dir/dir2\0";
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
    let subdir1 = b"/test_dir/dir1/subdir1\0";
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
    let fd = vfs_open(
        b"/test_dir/dir1/file1.txt\0".as_ptr() as *const c_char,
        O_CREAT | O_RDWR,
        0o755,
    );
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
    match verify_directory(b"/test_dir/dir1\0".as_ptr() as *const c_char) {
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
    let dir = vfs_open(path, O_RDONLY, 0o755);
    if dir < 0 {
        return Err(-ENOSYS);
    };

    let mut buf = [0u8; Dirent::SIZE * 4];
    // Print return value of each readdir call
    let count = vfs_getdents(dir, buf.as_mut_ptr() as *mut u8, buf.len());
    if count < 0 {
        vfs_close(dir);
        return Err(count);
    }
    for i in 0..count {
        let entry = unsafe { Dirent::from_buf(&buf[i as usize * Dirent::SIZE..]) };
        if entry.type_() == DirentType::Dir {
            println!(
                "[VFS Test DirctoryTree]: Found directory: {} {}",
                entry.ino(),
                entry.name()
            );
        } else {
            println!(
                "[VFS Test DirctoryTree]: Found file: {} {}",
                entry.ino(),
                entry.name()
            );
        }
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
        assert!(false);
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
        assert!(false);
    }
}

#[test]
#[cfg(procfs)]
fn vfs_test_procfs_posix() {
    // 1. Test: read /proc/meminfo
    let path_ptr = "/proc/meminfo";
    let fd = vfs_posix::open(path_ptr, O_RDONLY, 0o000);
    assert!(
        fd >= 0,
        "[VFS Test proc posix]  Failed to open file {}",
        path_ptr
    );
    let read_size = read_fd_content(path_ptr, fd);
    assert!(
        read_size > 0,
        "[VFS Test proc posix] Failed to read {}",
        path_ptr
    );
    vfs_close(fd);

    // 2. Test: read /proc/stat
    let path_ptr = "/proc/stat";
    let fd = vfs_posix::open(path_ptr, O_RDONLY, 0o000);
    assert!(
        fd >= 0,
        "[VFS Test proc posix]  Failed to open file {}",
        path_ptr
    );
    let read_size = read_fd_content(path_ptr, fd);
    assert!(
        read_size > 0,
        "[VFS Test proc posix] Failed to read {}",
        path_ptr
    );
    vfs_close(fd);

    // 3. Test: write the read-only file "/proc/meminfo"
    let path_ptr = "/proc/meminfo";
    let fd = vfs_posix::open(path_ptr, O_WRONLY, 0o000);
    assert!(
        fd == code::EACCES.to_errno(),
        "[VFS Test proc posix] The open operation should fail due to incorrect permissions"
    );
    let fd = vfs_posix::open(path_ptr, O_RDWR, 0o000);
    assert!(
        fd == code::EACCES.to_errno(),
        "[VFS Test proc posix] The open operation should fail due to incorrect permissions"
    );
    vfs_close(fd);

    // 4. Test: readdir /proc & read /proc/0/task/{tid}/task
    match vfs_posix::opendir("/proc") {
        Ok(dir) => {
            while let Ok(entry) = vfs_posix::readdir(&dir) {
                if &entry.name == "0" && entry.d_type == libc::DT_DIR {
                    let task_path = "/proc/0/task";
                    match vfs_posix::opendir(task_path) {
                        Ok(dir) => {
                            while let Ok(entry) = vfs_posix::readdir(&dir) {
                                if entry.d_type == libc::DT_DIR
                                    && entry.name_as_str() != "."
                                    && entry.name_as_str() != ".."
                                {
                                    println!(
                                        "[VFS Test proc posix]: Found /proc/0/task dir&file: {:?}",
                                        entry
                                    );
                                    let stat_path = &format!("{}/{}/status", task_path, entry.name);
                                    let fd = vfs_posix::open(stat_path, O_RDONLY, 0o000);
                                    assert!(
                                        fd >= 0,
                                        "[VFS Test proc posix]  Failed to open file {}",
                                        stat_path
                                    );
                                    let read_size = read_fd_content(stat_path, fd);
                                    assert!(
                                        read_size > 0,
                                        "[VFS Test proc posix] Failed to read {}",
                                        stat_path
                                    );
                                    vfs_close(fd);
                                }
                            }
                            vfs_posix::closedir(dir);
                        }
                        Err(err) => {
                            unreachable!(
                                "[VFS Test proc posix]: Opendir {} failed with error {}",
                                task_path, err
                            );
                        }
                    }
                }
            }
            vfs_posix::closedir(dir);
        }
        Err(err) => {
            unreachable!(
                "[VFS Test proc posix]: Opendir /proc failed with error {}",
                err
            );
        }
    }
}

#[test]
#[cfg(procfs)]
fn vfs_test_proc_internal_api() {
    // 1. Test: create a file in a non-existent directory
    if let Ok(_) = ProcFileSystem::proc_create_file("/proc/test/1", 0o444, None) {
        // There is no /proc/test path, so an error should be returned.
        unreachable!("[VFS Test proc api]: Success to crate file /proc/test/1, not as expected",);
    }
    // 2. Verify the existence of the folder after it is created
    if let Err(err) = ProcFileSystem::proc_mkdir("/proc/test", 0o555) {
        unreachable!("[VFS Test proc api]: Fail to mkdir /proc/test, err {}", err);
    }
    if let Err(err) = ProcFileSystem::proc_find("/proc/test") {
        unreachable!("[VFS Test proc api]: /proc/test not found, err {}", err);
    }
    // 3. Test: create a file in an existing directory
    if let Err(err) = ProcFileSystem::proc_create_file("/proc/test/1", 0o444, None) {
        unreachable!(
            "[VFS Test proc api]: Fail to crate file /proc/test/1, err {}",
            err
        );
    }
    // 4. Test: delete files
    if let Err(_) = ProcFileSystem::proc_remove("/proc/test") {
        unreachable!("[VFS Test proc api]: Fail to remove dir /proc/test",);
    }
    if let Ok(_) = ProcFileSystem::proc_find("/proc/test") {
        unreachable!("[VFS Test proc api]: /proc/test found, not as expected");
    }
}

fn read_fd_content(path: &str, fd: i32) -> usize {
    let mut read_buf;
    let mut read_size = 0;
    let mut result = String::new();
    loop {
        read_buf = [0u8; 64];
        let tmp_size = vfs_read(fd, read_buf.as_mut_ptr(), read_buf.len());
        let tmp: alloc::borrow::Cow<'_, str> =
            String::from_utf8_lossy(&read_buf[..tmp_size as usize]);
        result.push_str(tmp.as_ref());
        read_size += tmp_size;
        if tmp_size == 0 {
            break;
        }
    }
    println!("[VFS Test] read {} content:\n{}", path, result);
    read_size as usize
}
