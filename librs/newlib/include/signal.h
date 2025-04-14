#ifndef _LIBRS_SIGNAL_H
#define _LIBRS_SIGNAL_H

#include <bits/signal.h>
#include <stdint.h>
#include <sys/types.h>
#include <time.h>
#include <bits/pthread.h>
#include <features.h>

#define SIG_BLOCK 0

#define SIG_UNBLOCK 1

#define SIG_SETMASK 2

#define SI_QUEUE -1

#define SI_USER 0

#if defined(__linux__)
#define SIGHUP 1
#endif

#if defined(__redox__)
#define SIGHUP 1
#endif

#if defined(__linux__)
#define SIGINT 2
#endif

#if defined(__redox__)
#define SIGINT 2
#endif

#if defined(__linux__)
#define SIGQUIT 3
#endif

#if defined(__redox__)
#define SIGQUIT 3
#endif

#if defined(__linux__)
#define SIGILL 4
#endif

#if defined(__redox__)
#define SIGILL 4
#endif

#if defined(__linux__)
#define SIGTRAP 5
#endif

#if defined(__redox__)
#define SIGTRAP 5
#endif

#if defined(__linux__)
#define SIGABRT 6
#endif

#if defined(__redox__)
#define SIGABRT 6
#endif

#if defined(__linux__)
#define SIGIOT SIGABRT
#endif

#if defined(__linux__)
#define SIGBUS 7
#endif

#if defined(__redox__)
#define SIGBUS 7
#endif

#if defined(__linux__)
#define SIGFPE 8
#endif

#if defined(__redox__)
#define SIGFPE 8
#endif

#if defined(__linux__)
#define SIGKILL 9
#endif

#if defined(__redox__)
#define SIGKILL 9
#endif

#if defined(__linux__)
#define SIGUSR1 10
#endif

#if defined(__redox__)
#define SIGUSR1 10
#endif

#if defined(__linux__)
#define SIGSEGV 11
#endif

#if defined(__redox__)
#define SIGSEGV 11
#endif

#if defined(__linux__)
#define SIGUSR2 12
#endif

#if defined(__redox__)
#define SIGUSR2 12
#endif

#if defined(__linux__)
#define SIGPIPE 13
#endif

#if defined(__redox__)
#define SIGPIPE 13
#endif

#if defined(__linux__)
#define SIGALRM 14
#endif

#if defined(__redox__)
#define SIGALRM 14
#endif

#if defined(__linux__)
#define SIGTERM 15
#endif

#if defined(__redox__)
#define SIGTERM 15
#endif

#if defined(__linux__)
#define SIGSTKFLT 16
#endif

#if defined(__redox__)
#define SIGSTKFLT 16
#endif

#if defined(__linux__)
#define SIGCHLD 17
#endif

#if defined(__redox__)
#define SIGCHLD 17
#endif

#if defined(__linux__)
#define SIGCONT 18
#endif

#if defined(__redox__)
#define SIGCONT 18
#endif

#if defined(__linux__)
#define SIGSTOP 19
#endif

#if defined(__redox__)
#define SIGSTOP 19
#endif

#if defined(__linux__)
#define SIGTSTP 20
#endif

#if defined(__redox__)
#define SIGTSTP 20
#endif

#if defined(__linux__)
#define SIGTTIN 21
#endif

#if defined(__redox__)
#define SIGTTIN 21
#endif

#if defined(__linux__)
#define SIGTTOU 22
#endif

#if defined(__redox__)
#define SIGTTOU 22
#endif

#if defined(__linux__)
#define SIGURG 23
#endif

#if defined(__redox__)
#define SIGURG 23
#endif

#if defined(__linux__)
#define SIGXCPU 24
#endif

#if defined(__redox__)
#define SIGXCPU 24
#endif

#if defined(__linux__)
#define SIGXFSZ 25
#endif

#if defined(__redox__)
#define SIGXFSZ 25
#endif

#if defined(__linux__)
#define SIGVTALRM 26
#endif

#if defined(__redox__)
#define SIGVTALRM 26
#endif

#if defined(__linux__)
#define SIGPROF 27
#endif

#if defined(__redox__)
#define SIGPROF 27
#endif

#if defined(__linux__)
#define SIGWINCH 28
#endif

#if defined(__redox__)
#define SIGWINCH 28
#endif

#if defined(__linux__)
#define SIGIO 29
#endif

#if defined(__redox__)
#define SIGIO 29
#endif

#if defined(__linux__)
#define SIGPOLL SIGIO
#endif

#if defined(__linux__)
#define SIGPWR 30
#endif

#if defined(__redox__)
#define SIGPWR 30
#endif

#if defined(__linux__)
#define SIGSYS 31
#endif

#if defined(__redox__)
#define SIGSYS 31
#endif

#if defined(__linux__)
#define SIGUNUSED SIGSYS
#endif

#if defined(__linux__)
#define NSIG 32
#endif

#if defined(__redox__)
#define NSIG 32
#endif

#if defined(__linux__)
#define SIGRTMIN 35
#endif

#if defined(__redox__)
#define SIGRTMIN 35
#endif

#if defined(__linux__)
#define SIGRTMAX 64
#endif

#if defined(__redox__)
#define SIGRTMAX 64
#endif

#if defined(__linux__)
#define SA_NOCLDSTOP 1
#endif

#if defined(__redox__)
#define SA_NOCLDSTOP 1073741824
#endif

#if defined(__linux__)
#define SA_NOCLDWAIT 2
#endif

#if defined(__redox__)
#define SA_NOCLDWAIT 2
#endif

#if defined(__linux__)
#define SA_SIGINFO 4
#endif

#if defined(__redox__)
#define SA_SIGINFO 33554432
#endif

#if defined(__linux__)
#define SA_ONSTACK 134217728
#endif

#if defined(__redox__)
#define SA_ONSTACK 67108864
#endif

#if defined(__linux__)
#define SA_RESTART 268435456
#endif

#if defined(__redox__)
#define SA_RESTART 134217728
#endif

#if defined(__linux__)
#define SA_NODEFER 1073741824
#endif

#if defined(__redox__)
#define SA_NODEFER 268435456
#endif

#if defined(__linux__)
#define SA_RESETHAND 2147483648
#endif

#if defined(__redox__)
#define SA_RESETHAND 536870912
#endif

#if defined(__linux__)
#define SA_RESTORER 67108864
#endif

#if defined(__redox__)
#define SA_RESTORER 4
#endif

#if defined(__linux__)
#define SS_ONSTACK 1
#endif

#if defined(__redox__)
#define SS_ONSTACK 1
#endif

#if defined(__linux__)
#define SS_DISABLE 2
#endif

#if defined(__redox__)
#define SS_DISABLE 2
#endif

#if defined(__linux__)
#define MINSIGSTKSZ 2048
#endif

#if defined(__redox__)
#define MINSIGSTKSZ 2048
#endif

#if defined(__linux__)
#define SIGSTKSZ 8096
#endif

#if defined(__redox__)
#define SIGSTKSZ 8096
#endif

union sigval {
  int sival_int;
  void *sival_ptr;
};

struct siginfo {
  int si_signo;
  int si_errno;
  int si_code;
  pid_t si_pid;
  uid_t si_uid;
  void *si_addr;
  int si_status;
  union sigval si_value;
};

struct sigaltstack {
  void *ss_sp;
  int ss_flags;
  size_t ss_size;
};

typedef struct sigaltstack stack_t;

#if defined(__linux__)
struct _libc_fpxreg {
  uint16_t significand[4];
  uint16_t exponent;
  uint16_t __private[3];
};
#endif

#if defined(__linux__)
struct _libc_xmmreg {
  uint32_t element[4];
};
#endif

#if defined(__linux__)
struct _libc_fpstate {
  uint16_t cwd;
  uint16_t swd;
  uint16_t ftw;
  uint16_t fop;
  uint64_t rip;
  uint64_t rdp;
  uint32_t mxcsr;
  uint32_t mxcr_mask;
  struct _libc_fpxreg _st[8];
  struct _libc_xmmreg _xmm[16];
  uint64_t __private[12];
};
#endif

#if defined(__linux__)
struct mcontext {
  int64_t gregs[23];
  struct _libc_fpstate *fpregs;
  uint64_t __private[8];
};
#endif

#if defined(__redox__)
struct mcontext {
#if defined(__i386__)
  uint8_t _opaque[512]
#endif
  ;
#if defined(__x86_64__)
  uint8_t _opaque[864]
#endif
  ;
#if defined(__aarch64__)
  uint8_t _opaque[272]
#endif
  ;
#if defined(__riscv)
  uint8_t _opaque[520]
#endif
  ;
};
#endif

#if defined(__linux__)
typedef struct mcontext mcontext_t;
#endif

#if defined(__redox__)
typedef struct mcontext mcontext_t;
#endif

#if defined(__linux__)
struct ucontext {
  unsigned long uc_flags;
  ucontext_t *uc_link;
  stack_t uc_stack;
  mcontext_t uc_mcontext;
  sigset_t uc_sigmask;
  uint8_t __private[512];
};
#endif

#if defined(__redox__)
struct ucontext {
#if (defined(__x86_64__) || defined(__aarch64__) || defined(__riscv))
  uintptr_t _pad[1]
#endif
  ;
#if defined(__i386__)
  uintptr_t _pad[3]
#endif
  ;
  ucontext_t *uc_link;
  stack_t uc_stack;
  sigset_t uc_sigmask;
  uintptr_t _sival;
  uint32_t _sigcode;
  uint32_t _signum;
  mcontext_t uc_mcontext;
};
#endif

#if defined(__linux__)
typedef struct ucontext ucontext_t;
#endif

#if defined(__redox__)
typedef struct ucontext ucontext_t;
#endif

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

void _cbindgen_export_siginfo(struct siginfo a);

extern int32_t sigsetjmp(uint64_t *jb, int32_t savemask);

int32_t __sigsetjmp_tail(uint64_t *jb, int32_t ret);

void siglongjmp(uint64_t *jb, int32_t ret);

int kill(pid_t pid, int sig);

int sigqueue(pid_t pid, int sig, union sigval val);

int killpg(pid_t pgrp, int sig);

int pthread_kill(pthread_t thread, int sig);

int pthread_sigmask(int how, const sigset_t *set, sigset_t *oldset);

int raise(int sig);

int sigaction(int sig, const struct sigaction *act, struct sigaction *oact);

int sigaddset(sigset_t *set, int signo);

int sigaltstack(const stack_t *ss, stack_t *old_ss);

int sigdelset(sigset_t *set, int signo);

int sigemptyset(sigset_t *set);

int sigfillset(sigset_t *set);

int sighold(int sig);

int sigignore(int sig);

int siginterrupt(int sig, int flag);

int sigismember(const sigset_t *set, int signo);

void (*signal(int sig, void (*func)(int)))(int);

int sigpause(int sig);

int sigpending(sigset_t *set);

int sigprocmask(int how, const sigset_t *set, sigset_t *oset);

int sigrelse(int sig);

void (*sigset(int sig, void (*func)(int)))(int);

int sigsuspend(const sigset_t *sigmask);

int sigwait(const sigset_t *set, int *sig);

int sigtimedwait(const sigset_t *set, struct siginfo *sig, const struct timespec *tp);

int sigwaitinfo(const sigset_t *set, siginfo_t *sig);

void psignal(int sig, const char *prefix);

void psiginfo(const siginfo_t *info, const char *prefix);

#if defined(__redox__)
void __completely_unused_cbindgen_workaround_fn_ucontext_mcontext(const ucontext_t *a,
                                                                  const mcontext_t *b);
#endif

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_SIGNAL_H */

#include <bits/signal.h>
