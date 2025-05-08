#ifndef _LIBRS_PTHREAD_H
#define _LIBRS_PTHREAD_H

#include <sched.h>
#include <time.h>
#include <bits/pthread.h>
#include <features.h>

#define _POSIX_THREADS 1

#define PTHREAD_BARRIER_SERIAL_THREAD -1

#define PTHREAD_CANCEL_ASYNCHRONOUS 0

#define PTHREAD_CANCEL_ENABLE 1

#define PTHREAD_CANCEL_DEFERRED 2

#define PTHREAD_CANCEL_DISABLE 3

#define PTHREAD_CANCELED (void*)~0

#define PTHREAD_CREATE_DETACHED 0

#define PTHREAD_CREATE_JOINABLE 1

#define PTHREAD_EXPLICIT_SCHED 0

#define PTHREAD_INHERIT_SCHED 1

#define PTHREAD_MUTEX_DEFAULT 0

#define PTHREAD_MUTEX_ERRORCHECK 1

#define PTHREAD_MUTEX_NORMAL 2

#define PTHREAD_MUTEX_RECURSIVE 3

#define PTHREAD_MUTEX_ROBUST 0

#define PTHREAD_MUTEX_STALLED 1

#define PTHREAD_PRIO_INHERIT 0

#define PTHREAD_PRIO_NONE 0

#define PTHREAD_PRIO_PROTECT 0

#define PTHREAD_PROCESS_SHARED 0

#define PTHREAD_PROCESS_PRIVATE 1

#define PTHREAD_SCOPE_PROCESS 0

#define PTHREAD_SCOPE_SYSTEM 1

#define PTHREAD_KEYS_MAX (4096 * 32)

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

int pthread_cancel(pthread_t thread);

int pthread_create(pthread_t *pthread,
                   const pthread_attr_t *attr,
                   void *(*start_routine)(void *arg),
                   void *arg);

int pthread_detach(pthread_t pthread);

int pthread_equal(pthread_t pthread1, pthread_t pthread2);

__noreturn void pthread_exit(void *retval);

int pthread_getconcurrency(void);

int pthread_getcpuclockid(pthread_t thread, clockid_t *clock_out);

int pthread_getschedparam(pthread_t thread, int *policy_out, struct sched_param *param_out);

int pthread_join(pthread_t thread, void **retval);

pthread_t pthread_self(void);

int pthread_setcancelstate(int state, int *oldstate);

int pthread_setcanceltype(int ty, int *oldty);

int pthread_setconcurrency(int concurrency);

int pthread_setschedparam(pthread_t thread, int policy, const struct sched_param *param);

int pthread_setschedprio(pthread_t thread, int prio);

void pthread_testcancel(void);

void __librs_internal_pthread_cleanup_push(void *new_entry);

void __librs_internal_pthread_cleanup_pop(int execute);

int pthread_attr_destroy(pthread_attr_t *attr);

int pthread_attr_getdetachstate(const pthread_attr_t *attr, int *detachstate);

int pthread_attr_getguardsize(const pthread_attr_t *attr, size_t *size);

int pthread_attr_getinheritsched(const pthread_attr_t *attr, int *inheritsched);

int pthread_attr_getschedparam(const pthread_attr_t *attr, struct sched_param *param);

int pthread_attr_getschedpolicy(const pthread_attr_t *attr, int *policy);

int pthread_attr_getscope(const pthread_attr_t *attr, int *scope);

int pthread_attr_getstack(const pthread_attr_t *attr, void **stackaddr, size_t *stacksize);

int pthread_attr_getstacksize(const pthread_attr_t *attr, int *stacksize);

int pthread_attr_init(pthread_attr_t *attr);

int pthread_attr_setdetachstate(pthread_attr_t *attr, int detachstate);

int pthread_attr_setguardsize(pthread_attr_t *attr, int guardsize);

int pthread_attr_setinheritsched(pthread_attr_t *attr, int inheritsched);

int pthread_attr_setschedparam(pthread_attr_t *attr, const struct sched_param *param);

int pthread_attr_setschedpolicy(pthread_attr_t *attr, int policy);

int pthread_attr_setscope(pthread_attr_t *attr, int scope);

int pthread_attr_setstack(pthread_attr_t *attr, void *stackaddr, size_t stacksize);

int pthread_attr_setstacksize(pthread_attr_t *attr, size_t stacksize);

int pthread_barrier_destroy(pthread_barrier_t *barrier);

int pthread_barrier_init(pthread_barrier_t *barrier,
                         const pthread_barrierattr_t *attr,
                         unsigned int count);

int pthread_barrier_wait(pthread_barrier_t *barrier);

int pthread_barrierattr_init(pthread_barrierattr_t *attr);

int pthread_barrierattr_setpshared(pthread_barrierattr_t *attr, int pshared);

int pthread_barrierattr_getpshared(const pthread_barrierattr_t *attr, int *pshared);

int pthread_barrierattr_destroy(pthread_barrierattr_t *attr);

int pthread_cond_broadcast(pthread_cond_t *cond);

int pthread_cond_destroy(pthread_cond_t *cond);

int pthread_cond_init(pthread_cond_t *cond, const pthread_condattr_t *_attr);

int pthread_cond_signal(pthread_cond_t *cond);

int pthread_cond_timedwait(pthread_cond_t *cond,
                           pthread_mutex_t *mutex,
                           const struct timespec *timeout);

int pthread_cond_wait(pthread_cond_t *cond, pthread_mutex_t *mutex);

int pthread_condattr_destroy(pthread_condattr_t *condattr);

int pthread_condattr_getclock(const pthread_condattr_t *condattr, clockid_t *clock);

int pthread_condattr_getpshared(const pthread_condattr_t *condattr, int *pshared);

int pthread_condattr_init(pthread_condattr_t *condattr);

int pthread_condattr_setclock(pthread_condattr_t *condattr, clockid_t clock);

int pthread_condattr_setpshared(pthread_condattr_t *condattr, int pshared);

void *pthread_getspecific(pthread_key_t key);

int pthread_key_create(pthread_key_t *key_ptr, void (*destructor)(void *value));

int pthread_key_delete(pthread_key_t key);

int pthread_setspecific(pthread_key_t key, const void *value);

int pthread_mutex_consistent(pthread_mutex_t *mutex);

int pthread_mutex_destroy(pthread_mutex_t *mutex);

int pthread_mutex_getprioceiling(const pthread_mutex_t *mutex, int *prioceiling);

int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr);

int pthread_mutex_lock(pthread_mutex_t *mutex);

int pthread_mutex_setprioceiling(pthread_mutex_t *mutex, int prioceiling, int *old_prioceiling);

int pthread_mutex_timedlock(pthread_mutex_t *mutex, const struct timespec *timespec);

int pthread_mutex_trylock(pthread_mutex_t *mutex);

int pthread_mutex_unlock(pthread_mutex_t *mutex);

int pthread_mutexattr_destroy(pthread_mutexattr_t *attr);

int pthread_mutexattr_getprioceiling(const pthread_mutexattr_t *attr, int *prioceiling);

int pthread_mutexattr_getprotocol(const pthread_mutexattr_t *attr, int *protocol);

int pthread_mutexattr_getpshared(const pthread_mutexattr_t *attr, int *pshared);

int pthread_mutexattr_getrobust(const pthread_mutexattr_t *attr, int *robust);

int pthread_mutexattr_gettype(const pthread_mutexattr_t *attr, int *ty);

int pthread_mutexattr_init(pthread_mutexattr_t *attr);

int pthread_mutexattr_setprioceiling(pthread_mutexattr_t *attr, int prioceiling);

int pthread_mutexattr_setprotocol(pthread_mutexattr_t *attr, int protocol);

int pthread_mutexattr_setpshared(pthread_mutexattr_t *attr, int pshared);

int pthread_mutexattr_setrobust(pthread_mutexattr_t *attr, int robust);

int pthread_mutexattr_settype(pthread_mutexattr_t *attr, int ty);

int pthread_once(pthread_once_t *once, void (*constructor)(void));

int pthread_rwlock_init(pthread_rwlock_t *rwlock, const pthread_rwlockattr_t *attr);

int pthread_rwlock_rdlock(pthread_rwlock_t *rwlock);

int pthread_rwlock_timedrdlock(pthread_rwlock_t *rwlock, const struct timespec *timeout);

int pthread_rwlock_timedwrlock(pthread_rwlock_t *rwlock, const struct timespec *timeout);

int pthread_rwlock_tryrdlock(pthread_rwlock_t *rwlock);

int pthread_rwlock_trywrlock(pthread_rwlock_t *rwlock);

int pthread_rwlock_unlock(pthread_rwlock_t *rwlock);

int pthread_rwlock_wrlock(pthread_rwlock_t *rwlock);

int pthread_rwlockattr_init(pthread_rwlockattr_t *attr);

int pthread_rwlockattr_getpshared(const pthread_rwlockattr_t *attr, int *pshared_out);

int pthread_rwlockattr_setpshared(pthread_rwlockattr_t *attr, int pshared);

int pthread_rwlockattr_destroy(pthread_rwlockattr_t *attr);

int pthread_rwlock_destroy(pthread_rwlock_t *rwlock);

int pthread_spin_destroy(pthread_spinlock_t *spinlock);

int pthread_spin_init(pthread_spinlock_t *spinlock, int _pshared);

int pthread_spin_lock(pthread_spinlock_t *spinlock);

int pthread_spin_trylock(pthread_spinlock_t *spinlock);

int pthread_spin_unlock(pthread_spinlock_t *spinlock);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_PTHREAD_H */
