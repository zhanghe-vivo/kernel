#ifndef _LIBRS_UNISTD_H
#define _LIBRS_UNISTD_H

#include <stddef.h>
#include <stdint.h>
#include <sys/types.h>
#include <features.h>

#define F_OK 0

#define R_OK 4

#define W_OK 2

#define X_OK 1

#define SEEK_SET 0

#define SEEK_CUR 1

#define SEEK_END 2

#define F_ULOCK 0

#define F_LOCK 1

#define F_TLOCK 2

#define F_TEST 3

#define STDIN_FILENO 0

#define STDOUT_FILENO 1

#define STDERR_FILENO 2

#define L_cuserid 9

#define _PC_LINK_MAX 0

#define _PC_MAX_CANON 1

#define _PC_MAX_INPUT 2

#define _PC_NAME_MAX 3

#define _PC_PATH_MAX 4

#define _PC_PIPE_BUF 5

#define _PC_CHOWN_RESTRICTED 6

#define _PC_NO_TRUNC 7

#define _PC_VDISABLE 8

#define _PC_SYNC_IO 9

#define _PC_ASYNC_IO 10

#define _PC_PRIO_IO 11

#define _PC_SOCK_MAXBUF 12

#define _PC_FILESIZEBITS 13

#define _PC_REC_INCR_XFER_SIZE 14

#define _PC_REC_MAX_XFER_SIZE 15

#define _PC_REC_MIN_XFER_SIZE 16

#define _PC_REC_XFER_ALIGN 17

#define _PC_ALLOC_SIZE_MIN 18

#define _PC_SYMLINK_MAX 19

#define _PC_2_SYMLINKS 20

#define _SC_ARG_MAX 0

#define _SC_CHILD_MAX 1

#define _SC_CLK_TCK 2

#define _SC_NGROUPS_MAX 3

#define _SC_OPEN_MAX 4

#define _SC_STREAM_MAX 5

#define _SC_TZNAME_MAX 6

#define _SC_VERSION 29

#define _SC_PAGESIZE 30

#define _SC_PAGE_SIZE 30

#define _SC_RE_DUP_MAX 44

#define _SC_NPROCESSORS_ONLN 58

#define _SC_GETGR_R_SIZE_MAX 69

#define _SC_GETPW_R_SIZE_MAX 70

#define _SC_LOGIN_NAME_MAX 71

#define _SC_TTY_NAME_MAX 72

#define _SC_SYMLOOP_MAX 173

#define _SC_HOST_NAME_MAX 180

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getopt.html>.
 */
extern char *optarg;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getopt.html>.
 */
extern int opterr;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getopt.html>.
 */
extern int optind;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getopt.html>.
 */
extern int optopt;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/_Exit.html>.
 */
void _exit(int status) __noreturn;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/access.html>.
 */
int access(const char *path, int mode);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/alarm.html>.
 */
unsigned int alarm(unsigned int seconds);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/chdir.html>.
 */
int chdir(const char *path);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/chown.html>.
 */
int chown(const char *path, uid_t owner, gid_t group);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/chroot.html>.
 *
 * # Deprecation
 * The `chroot()` function was marked legacy in the System Interface & Headers
 * Issue 5, and removed in Issue 6.
 */
__deprecated int chroot(const char *path);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/close.html>.
 */
int close(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/crypt.html>.
 */
char *crypt(const char *key, const char *salt);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/daemon.3.html>.
 */
int daemon(int nochdir, int noclose);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/dup.html>.
 */
int dup(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/dup.html>.
 */
int dup2(int fildes, int fildes2);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execl(const char *path, const char *arg0, ...);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execle(const char *path, const char *arg0, ...);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execlp(const char *file, const char *arg0, ...);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execv(const char *path, char *const *argv);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execve(const char *path, char *const *argv, char *const *envp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exec.html>.
 */
int execvp(const char *file, char *const *argv);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchdir.html>.
 */
int fchdir(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchown.html>.
 */
int fchown(int fildes, uid_t owner, gid_t group);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fdatasync.html>.
 */
int fdatasync(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fork.html>.
 */
pid_t fork(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fsync.html>.
 */
int fsync(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ftruncate.html>.
 */
int ftruncate(int fildes, off_t length);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getcwd.html>.
 */
char *getcwd(char *buf, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/getdtablesize.html>.
 *
 * # Deprecation
 * The `getdtablesize()` function was marked legacy in the System Interface &
 * Headers Issue 5, and removed in Issue 6.
 */
__deprecated int getdtablesize(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getegid.html>.
 */
gid_t getegid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/geteuid.html>.
 */
uid_t geteuid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getgid.html>.
 */
gid_t getgid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getgroups.html>.
 */
int getgroups(int size, gid_t *list);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/gethostname.html>.
 */
int gethostname(char *name, size_t len);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getlogin.html>.
 */
char *getlogin(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getlogin.html>.
 */
int getlogin_r(char *name, size_t namesize);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/getpagesize.html>.
 *
 * # Deprecation
 * The `getpagesize()` function was marked legacy in the System Interface &
 * Headers Issue 5, and removed in Issue 6.
 */
__deprecated int getpagesize(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpgid.html>.
 */
pid_t getpgid(pid_t pid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpgrp.html>.
 */
pid_t getpgrp(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getpid.html>.
 */
pid_t getpid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getppid.html>.
 */
pid_t getppid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getsid.html>.
 */
pid_t getsid(pid_t pid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getuid.html>.
 */
uid_t getuid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/functions/getwd.html>.
 *
 * # Deprecation
 * The `getwd()` function was marked legacy in the Open Group Base
 * Specifications Issue 6, and removed in Issue 7.
 */
__deprecated char *getwd(char *path_name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/isatty.html>.
 */
int isatty(int fd);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/lchown.html>.
 */
int lchown(const char *path, uid_t owner, gid_t group);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/link.html>.
 */
int link(const char *path1, const char *path2);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/lockf.html>.
 */
int lockf(int fildes, int function, off_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/lseek.html>.
 */
off_t lseek(int fildes, off_t offset, int whence);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/pipe.html>.
 */
int pipe(int *fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/pipe.html>.
 */
int pipe2(int *fildes, int flags);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/read.html>.
 */
ssize_t pread(int fildes, void *buf, size_t nbyte, off_t offset);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/pthread_atfork.html>.
 *
 * TODO: specified in `pthread.h` in modern POSIX
 */
int pthread_atfork(void (*prepare)(void), void (*parent)(void), void (*child)(void));

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/write.html>.
 */
ssize_t pwrite(int fildes, const void *buf, size_t nbyte, off_t offset);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/read.html>.
 */
ssize_t read(int fildes, const void *buf, size_t nbyte);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/readlink.html>.
 */
ssize_t readlink(const char *path, char *buf, size_t bufsize);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/rmdir.html>.
 */
int rmdir(const char *path);

int set_default_scheme(const char *scheme);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setgid.html>.
 */
int setgid(gid_t gid);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man2/setgroups.2.html>.
 *
 * TODO: specified in `grp.h`?
 */
int setgroups(size_t size, const gid_t *list);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setpgid.html>.
 */
int setpgid(pid_t pid, pid_t pgid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/setpgrp.html>.
 *
 * # Deprecation
 * The `setpgrp()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated pid_t setpgrp(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setregid.html>.
 */
int setregid(gid_t rgid, gid_t egid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setresgid.html>.
 */
int setresgid(gid_t rgid, gid_t egid, gid_t sgid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setresuid.html>.
 */
int setresuid(uid_t ruid, uid_t euid, uid_t suid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setreuid.html>.
 */
int setreuid(uid_t ruid, uid_t euid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setsid.html>.
 */
pid_t setsid(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setuid.html>.
 */
int setuid(uid_t uid);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sleep.html>.
 */
unsigned int sleep(unsigned int seconds);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/swab.html>.
 */
void swab(const void *src, void *dest, ssize_t nbytes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/symlink.html>.
 */
int symlink(const char *path1, const char *path2);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sync.html>.
 */
void sync(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcgetpgrp.html>.
 */
pid_t tcgetpgrp(int fd);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tcsetpgrp.html>.
 */
int tcsetpgrp(int fd, pid_t pgrp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/truncate.html>.
 */
int truncate(const char *path, off_t length);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ttyname.html>.
 */
char *ttyname(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ttyname.html>.
 */
int ttyname_r(int fildes, char *name, size_t namesize);

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/functions/ualarm.html>.
 *
 * # Deprecation
 * The `ualarm()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 6, and removed in Issue 7.
 */
__deprecated useconds_t ualarm(useconds_t usecs, useconds_t interval);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/unlink.html>.
 */
int unlink(const char *path);

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/functions/usleep.html>.
 *
 * # Deprecation
 * The `usleep()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 6, and removed in Issue 7.
 */
__deprecated int usleep(useconds_t useconds);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/write.html>.
 */
ssize_t write(int fildes, const void *buf, size_t nbyte);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/brk.html>.
 *
 * # Deprecation
 * The `brk()` function was marked legacy in the System Interface & Headers
 * Issue 5, and removed in Issue 6.
 */
__deprecated int brk(void *addr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/brk.html>.
 *
 * # Deprecation
 * The `sbrk()` function was marked legacy in the System Interface & Headers
 * Issue 5, and removed in Issue 6.
 */
__deprecated void *sbrk(intptr_t incr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getopt.html>.
 */
int getopt(int argc, char *const *argv, const char *optstring);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/getpass.html>.
 *
 * # Deprecation
 * The `getpass()` function was marked legacy in the Open Group System
 * Interface & Headers Issue 5, and removed in Issue 6.
 */
__deprecated char *getpass(const char *prompt);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fpathconf.html>.
 */
long fpathconf(int _fildes, int name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fpathconf.html>.
 */
long pathconf(const char *_path, int name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sysconf.html>.
 */
long sysconf(int name);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_UNISTD_H */

#include <bits/fcntl.h>
#include <bits/unistd.h>
