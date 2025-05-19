use bluekernel::{
    println,
    vfs::{vfs_api::*, vfs_mode::*, vfs_posix},
};
use bluekernel_test_macro::test;
use core::ffi::{c_char, c_int, CStr};
use libc::{ENOSYS, O_CREAT, O_RDWR, SEEK_SET};

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
    let root_dir_str = unsafe { CStr::from_ptr(root_dir.as_ptr() as *const c_char) }
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(root_dir_str, 0o755);
    assert!(
        result >= 0,
        "[VFS Test DirctoryTree]: Failed to create root test directory"
    );

    // Create subdirectory dir1
    let dir1 = b"/test_dir/dir1\0";
    let dir1_str = unsafe { CStr::from_ptr(dir1.as_ptr() as *const c_char) }
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(dir1_str, 0o755);
    if result < 0 {
        vfs_posix::rmdir(root_dir_str);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory dir1: {}",
            result
        );
    }

    // Create subdirectory dir2
    let dir2 = b"/test_dir/dir2\0";
    let dir2_str = unsafe { CStr::from_ptr(dir2.as_ptr() as *const c_char) }
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(dir2_str, 0o755);
    if result < 0 {
        vfs_posix::rmdir(dir1_str);
        vfs_posix::rmdir(root_dir_str);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory dir2: {}",
            result
        );
    }

    // Create subdirectory subdir1
    let subdir1 = b"/test_dir/dir1/subdir1\0";
    let subdir1_str = unsafe { CStr::from_ptr(subdir1.as_ptr() as *const c_char) }
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(subdir1_str, 0o755);
    if result < 0 {
        vfs_posix::rmdir(dir1_str);
        vfs_posix::rmdir(dir2_str);
        vfs_posix::rmdir(root_dir_str);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create directory subdir1: {}",
            result
        );
    }

    // Create test file
    let fd = vfs_posix::open("/test_dir/dir1/file1.txt", O_CREAT | O_RDWR, 0o755);
    if fd < 0 {
        vfs_posix::rmdir(subdir1_str);
        vfs_posix::rmdir(dir1_str);
        vfs_posix::rmdir(dir2_str);
        vfs_posix::rmdir(root_dir_str);
        unreachable!(
            "[VFS Test DirctoryTree]: Failed to create test file: {}",
            fd
        );
    }
    vfs_posix::close(fd);

    // Verify directory structure
    unsafe {
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
}

unsafe fn verify_directory(path: *const c_char) -> Result<(), c_int> {
    let path_str = CStr::from_ptr(path).to_str().unwrap();
    let Ok(dir) = vfs_posix::opendir(path_str) else {
        return Err(-ENOSYS);
    };

    // Print return value of each readdir call
    while let Ok(entry) = vfs_posix::readdir(&dir) {
        if entry.d_type == libc::DT_DIR {
            println!("[VFS Test DirctoryTree]: Found directory: {:?}", entry);
        } else {
            println!("[VFS Test DirctoryTree]: Found file: {:?}", entry);
        }
    }

    // Close directory
    vfs_posix::closedir(dir);
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
