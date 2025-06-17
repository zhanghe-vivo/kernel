#ifndef _LIBRS_FCNTL_H
#define _LIBRS_FCNTL_H

#include <stdarg.h>
#include <sys/types.h>

#define F_DUPFD 0

#define F_GETFD 1

#define F_SETFD 2

#define F_GETFL 3

#define F_SETFL 4

#define F_GETLK 5

#define F_SETLK 6

#define F_SETLKW 7

#define F_RDLCK 0

#define F_WRLCK 1

#define F_UNLCK 2

#define F_ULOCK 0

#define F_LOCK 1

#define F_TLOCK 2

#define F_TEST 3

#if defined(__linux__)
#define O_RDONLY 0
#endif

#if defined(__blueos__)
#define O_RDONLY 0
#endif

#if defined(__linux__)
#define O_WRONLY 1
#endif

#if defined(__blueos__)
#define O_WRONLY 1
#endif

#if defined(__linux__)
#define O_RDWR 2
#endif

#if defined(__blueos__)
#define O_RDWR 2
#endif

#if defined(__linux__)
#define O_ACCMODE 3
#endif

#if defined(__blueos__)
#define O_ACCMODE 3
#endif

#if defined(__linux__)
#define O_CREAT 64
#endif

#if defined(__blueos__)
#define O_CREAT 64
#endif

#if defined(__linux__)
#define O_EXCL 128
#endif

#if defined(__blueos__)
#define O_EXCL 128
#endif

#if defined(__linux__)
#define O_TRUNC 512
#endif

#if defined(__blueos__)
#define O_TRUNC 512
#endif

#if defined(__linux__)
#define O_APPEND 1024
#endif

#if defined(__blueos__)
#define O_APPEND 1024
#endif

#if defined(__linux__)
#define O_NONBLOCK 2048
#endif

#if defined(__blueos__)
#define O_NONBLOCK 2048
#endif

#if defined(__linux__)
#define O_DIRECTORY 65536
#endif

#if defined(__blueos__)
#define O_DIRECTORY 65536
#endif

#if defined(__linux__)
#define O_NOFOLLOW 131072
#endif

#if defined(__blueos__)
#define O_NOFOLLOW 131072
#endif

#if defined(__linux__)
#define O_CLOEXEC 524288
#endif

#if defined(__blueos__)
#define O_CLOEXEC 524288
#endif

#if defined(__linux__)
#define O_PATH 2097152
#endif

#if defined(__blueos__)
#define O_PATH 2097152
#endif

#if defined(__linux__)
#define FD_CLOEXEC 524288
#endif

#if defined(__blueos__)
#define FD_CLOEXEC 524288
#endif

#if defined(__blueos__)
#define O_SHLOCK 1048576
#endif

#if defined(__blueos__)
#define O_EXLOCK 2097152
#endif

#if defined(__blueos__)
#define O_ASYNC 4194304
#endif

#if defined(__blueos__)
#define O_FSYNC 8388608
#endif

#if defined(__blueos__)
#define O_SYNC O_FSYNC
#endif

#if defined(__blueos__)
#define O_SYMLINK 1073741824
#endif

#if defined(__blueos__)
#define O_NOCTTY 512
#endif

struct flock {
  short l_type;
  short l_whence;
  off_t l_start;
  off_t l_len;
  pid_t l_pid;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

int creat(const char *path, mode_t mode);

int fcntl(int fildes, int cmd, ...);

int open(const char *path, int oflag, ...);

void cbindgen_stupid_struct_user_for_fcntl(struct flock a);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_FCNTL_H */

#include <bits/fcntl.h>
