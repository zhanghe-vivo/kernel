//! vfs_test.rs  

use crate::{
    thread::Threaed,
    vfs::{vfs_api::*, vfs_log::*, vfs_mode::*, vfs_posix},
};
use alloc::slice;
use core::ffi::{c_char, c_int, CStr};

#[no_mangle]
pub unsafe extern "C" fn vfs_test() -> c_int {
    vfs_test_directory_tree();
    vfs_test_read_and_write();
    vfs_test_multiple_open();
    vfs_test_uart();
    0
}

pub unsafe fn vfs_test_uart() -> c_int {
    // Test UART device path
    let uart_path = b"/dev/uart0\0";
    let path_ptr = uart_path.as_ptr() as *const c_char;

    vfslog!("[VFS Test devfs]: Starting...");
    vfslog!("[VFS Test devfs]: Opening device /dev/uart0");

    let fd = vfs_open(path_ptr, O_RDWR, 0);
    if fd < 0 {
        vfslog!(
            "[VFS Test devfs]: Failed to open UART device, error = {}",
            fd
        );
        return fd;
    }

    vfslog!(
        "[VFS Test devfs]: Successfully opened UART device, fd = {}",
        fd
    );

    let test_data = b"UART Test Message\n";
    let write_size = vfs_write(fd, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfslog!(
            "[VFS Test devfs]: Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
        vfs_close(fd);
        return -1;
    }
    vfslog!("[VFS Test devfs]: Successfully wrote {} bytes", write_size);
    let mut read_buf = [0u8; 64];
    vfslog!("[VFS Test devfs]: Attempting to read data (timeout: 3s)...");

    let mut total_read = 0;
    let mut attempts = 0;
    const MAX_ATTEMPTS: i32 = 30; // 3 seconds = 30 * 100ms

    while attempts < MAX_ATTEMPTS {
        let read_size = vfs_read(
            fd,
            read_buf[total_read..].as_mut_ptr(),
            read_buf.len() - total_read,
        );

        if read_size < 0 {
            vfslog!("[VFS Test devfs]: Read failed, error = {}", read_size);
            vfs_close(fd);
            return read_size as i32;
        }

        if read_size > 0 {
            total_read += read_size as usize;
            vfslog!("[VFS Test devfs]: Read {} bytes in this attempt", read_size);

            let received_data =
                core::str::from_utf8(slice::from_raw_parts(read_buf.as_ptr(), total_read));
            match received_data {
                Ok(s) => vfslog!("[VFS Test devfs]: Current received data: {}", s),
                Err(_) => vfslog!("[VFS Test devfs]: Current received non-UTF8 data"),
            }

            if total_read >= read_buf.len() || read_buf[total_read - 1] == b'\n' {
                break;
            }
        }

        Thread::msleep(100);
        attempts += 1;

        if attempts % 10 == 0 {
            vfslog!(
                "[VFS Test devfs]: Waiting for data... ({}/3 seconds)",
                attempts / 10
            );
        }
    }

    if total_read > 0 {
        vfslog!("[VFS Test devfs]: Total read {} bytes", total_read);
        let received_data =
            core::str::from_utf8(slice::from_raw_parts(read_buf.as_ptr(), total_read));
        match received_data {
            Ok(s) => vfslog!("[VFS Test devfs]: Final received data: {}", s),
            Err(_) => vfslog!("[VFS Test devfs]: Final received non-UTF8 data"),
        }
    } else {
        vfslog!("[VFS Test devfs]: No data received after 3 seconds");
    }

    let close_result = vfs_close(fd);
    if close_result < 0 {
        vfslog!(
            "[VFS Test devfs]: Failed to close device, error = {}",
            close_result
        );
        return close_result;
    }

    vfslog!("[VFS Test devfs]: Device closed successfully");
    vfslog!("[VFS Test devfs]: All tests completed");
    0
}

#[no_mangle]
pub unsafe fn vfs_test_read_and_write() -> c_int {
    // Test file path
    let test_path = b"/test.txt\0";
    let path_ptr = test_path.as_ptr() as *const c_char;

    // Open flags: create and read/write
    let flags = O_CREAT | O_RDWR;

    // Default file permissions: 644
    let mode: mode_t = 0o644;

    vfslog!("[VFS Test Read/Write]  Starting...");
    vfslog!("[VFS Test Read/Write]  Opening file /test.txt");

    let fd = vfs_open(path_ptr, flags, mode);
    if fd < 0 {
        vfslog!("[VFS Test Read/Write]  Failed to open file, error = {}", fd);
        return fd;
    }

    vfslog!(
        "[VFS Test Read/Write]  Successfully opened file, fd = {}",
        fd
    );

    let test_data = b"Hello, RT-Thread!\n";
    let write_size = vfs_write(fd, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfslog!(
            "[VFS Test Read/Write]  Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
        vfs_close(fd);
        return -1;
    }
    vfslog!(
        "[VFS Test Read/Write]  Successfully wrote {} bytes",
        write_size
    );

    // Move file pointer back to start
    let seek_result = vfs_lseek(fd, 0, SEEK_SET);
    if seek_result < 0 {
        vfslog!(
            "[VFS Test Read/Write]  Seek failed, error = {}",
            seek_result
        );
        vfs_close(fd);
        return seek_result as i32;
    }

    // Read data and verify
    let mut read_buf = [0u8; 64];
    let read_size = vfs_read(fd, read_buf.as_mut_ptr(), test_data.len());
    if read_size != test_data.len() as isize {
        vfslog!(
            "[VFS Test Read/Write]  Read failed, expected {} bytes, read {}",
            test_data.len(),
            read_size
        );
        vfs_close(fd);
        return -1;
    }
    vfslog!(
        "[VFS Test Read/Write]  Successfully read {} bytes",
        read_size
    );

    // Verify read data
    let read_data = &read_buf[..test_data.len()];
    if read_data != test_data {
        vfslog!("[VFS Test Read/Write]  Data verification failed");
        vfslog!("[VFS Test Read/Write]  Expected: {:?}", test_data);
        vfslog!("[VFS Test Read/Write]  Got: {:?}", read_data);
        vfs_close(fd);
        return -1;
    }
    vfslog!("[VFS Test Read/Write]  Data verification successful");

    if fd > 0 {
        vfs_close(fd);
    }

    vfslog!("[VFS Test Read/Write]  Test completed successfully");
    0
}

#[no_mangle]
pub unsafe fn vfs_test_multiple_open() -> c_int {
    // Test file path
    let test_path = b"/test_multi.txt\0";
    let path_ptr = test_path.as_ptr() as *const c_char;

    vfslog!("VFS Test Multiple Open: Starting...");

    // First open for writing
    let fd1 = vfs_open(path_ptr, O_CREAT | O_RDWR, 0o644);
    if fd1 < 0 {
        vfslog!(
            "[VFS Test MultiOpen]: Failed to open first fd, error = {}",
            fd1
        );
        return fd1;
    }
    vfslog!("[VFS Test MultiOpen]: Opened first fd = {}", fd1);

    // Write test data
    let test_data = b"Hello, Multiple Open Test!\n";
    let write_size = vfs_write(fd1, test_data.as_ptr(), test_data.len());
    if write_size != test_data.len() as isize {
        vfslog!(
            "[VFS Test MultiOpen]: Write failed, expected {} bytes, wrote {}",
            test_data.len(),
            write_size
        );
        vfs_close(fd1);
        return -1;
    }
    vfslog!(
        "[VFS Test MultiOpen]: Wrote {} bytes through fd1",
        write_size
    );

    // Second open for reading
    let fd2 = vfs_open(path_ptr, O_RDWR, 0o644);
    if fd2 < 0 {
        vfslog!(
            "[VFS Test MultiOpen]: Failed to open second fd, error = {}",
            fd2
        );
        vfs_close(fd1);
        return fd2;
    }
    vfslog!("[VFS Test MultiOpen]: Opened second fd = {}", fd2);

    // Read data through second file descriptor
    let mut read_buf = [0u8; 64];
    let read_size = vfs_read(fd2, read_buf.as_mut_ptr(), test_data.len());
    if read_size != test_data.len() as isize {
        vfslog!(
            "[VFS Test MultiOpen]: Read failed, expected {} bytes, read {}",
            test_data.len(),
            read_size
        );
        vfs_close(fd1);
        vfs_close(fd2);
        return -1;
    }
    vfslog!("[VFS Test MultiOpen]: Read {} bytes through fd2", read_size);

    // Verify read data
    let read_data = &read_buf[..test_data.len()];
    if read_data != test_data {
        vfslog!("[VFS Test MultiOpen]: Data verification failed");
        vfslog!("[VFS Test MultiOpen]: Expected: {:?}", test_data);
        vfslog!("[VFS Test MultiOpen]: Got: {:?}", read_data);
        vfs_close(fd1);
        vfs_close(fd2);
        return -1;
    }
    vfslog!("[VFS Test MultiOpen]: Data verification successful");

    // Close file descriptors
    vfs_close(fd1);
    vfs_close(fd2);

    vfslog!("[VFS Test MultiOpen]: All tests passed successfully");
    0
}

#[no_mangle]
pub unsafe fn vfs_test_directory_tree() -> c_int {
    vfslog!("VFS Directory Tree Test: Starting...");

    // Create test directory structure:
    // /test_dir
    // /test_dir/dir1
    // /test_dir/dir2
    // /test_dir/dir1/subdir1
    // /test_dir/dir1/file1.txt

    // Create root test directory
    let root_dir = b"/test_dir\0";
    let root_dir_str = CStr::from_ptr(root_dir.as_ptr() as *const c_char)
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(root_dir_str, 0o755);
    if result < 0 {
        vfslog!(
            "[VFS Test DirctoryTree]: Failed to create root test directory: {}",
            result
        );
        return result;
    }
    vfslog!("[VFS Test DirctoryTree]: Created root directory: /test_dir");

    // Create subdirectory dir1
    let dir1 = b"/test_dir/dir1\0";
    let dir1_str = CStr::from_ptr(dir1.as_ptr() as *const c_char)
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(dir1_str, 0o755);
    if result < 0 {
        vfslog!(
            "[VFS Test DirctoryTree]: Failed to create directory dir1: {}",
            result
        );
        return result;
    }
    vfslog!("[VFS Test DirctoryTree]: Created directory: /test_dir/dir1");

    // Create subdirectory dir2
    let dir2 = b"/test_dir/dir2\0";
    let dir2_str = CStr::from_ptr(dir2.as_ptr() as *const c_char)
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(dir2_str, 0o755);
    if result < 0 {
        vfslog!("Failed to create directory dir2: {}", result);
        return result;
    }
    vfslog!("[VFS Test DirctoryTree]: Created directory: /test_dir/dir2");

    // Create subdirectory subdir1
    let subdir1 = b"/test_dir/dir1/subdir1\0";
    let subdir1_str = CStr::from_ptr(subdir1.as_ptr() as *const c_char)
        .to_str()
        .unwrap();
    let result = vfs_posix::mkdir(subdir1_str, 0o755);
    if result < 0 {
        vfslog!("Failed to create directory subdir1: {}", result);
        return result;
    }
    vfslog!("[VFS Test DirctoryTree]: Created directory: /test_dir/dir1/subdir1");

    // Create test file
    let test_file = b"/test_dir/dir1/file1.txt\0";
    let test_file_ptr = test_file.as_ptr() as *const c_char;
    let fd = vfs_posix::open(test_file_ptr, O_CREAT | O_RDWR, 0o644);
    if fd < 0 {
        vfslog!("Failed to create test file: {}", fd);
        return fd;
    }
    vfs_posix::close(fd);
    vfslog!("[VFS Test DirctoryTree]: Created file: /test_dir/dir1/file1.txt");

    // Verify directory structure
    vfslog!("[VFS Test DirctoryTree]: Verifying directory structure:");
    match verify_directory(b"/test_dir/dir1\0".as_ptr() as *const c_char) {
        Ok(_) => {
            vfslog!("[VFS Test DirctoryTree]:  All tests passed successfully");
            0
        }
        Err(err) => {
            vfslog!(
                "[VFS Test DirctoryTree]:  Verification failed with error {}",
                err
            );
            err
        }
    }
}

unsafe fn verify_directory(path: *const c_char) -> Result<(), c_int> {
    let path_str = CStr::from_ptr(path).to_str().unwrap();
    let dir = vfs_posix::opendir(path_str);
    if dir.is_none() {
        vfslog!(
            "[VFS Test DirctoryTree]: Failed to open directory: {}",
            path_str
        );
        return Err(-1);
    }
    let dir = dir.unwrap();

    vfslog!(
        "\n [VFS Test DirctoryTree]: Listing directory: \"{}\"",
        path_str
    );

    // Print return value of each readdir call
    while let Some(entry) = vfs_posix::readdir(&dir) {
        vfslog!("[VFS Test DirctoryTree]: Found entry: {:?}", entry);
    }

    // Close directory
    vfs_posix::closedir(dir);
    Ok(())
}
