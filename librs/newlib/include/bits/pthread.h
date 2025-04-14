#ifndef _LIBRS_BITS_PTHREAD_H
#define _LIBRS_BITS_PTHREAD_H

#include <bits/sched.h>
#define PTHREAD_COND_INITIALIZER ((pthread_cond_t){0})
#define PTHREAD_MUTEX_INITIALIZER ((pthread_mutex_t){0})
#define PTHREAD_ONCE_INIT ((pthread_once_t){0})
#define PTHREAD_RWLOCK_INITIALIZER ((pthread_rwlock_t){0})

#define pthread_cleanup_push(ROUTINE, ARG) do { \
  struct { \
    void (*routine)(void *); \
    void *arg; \
    void *prev; \
  } __librs_internal_pthread_ll_entry = { \
    .routine = (void (*)(void *))(ROUTINE), \
    .arg = (void *)(ARG), \
  }; \
  __librs_internal_pthread_cleanup_push(&__librs_internal_pthread_ll_entry);

#define pthread_cleanup_pop(EXECUTE) \
  __librs_internal_pthread_cleanup_pop((EXECUTE)); \
} while(0)



typedef union {
  unsigned char __librs_internal_size[32];
  size_t __librs_internal_align;
} pthread_attr_t;

typedef union {
  unsigned char __librs_internal_size[1];
  unsigned char __librs_internal_align;
} pthread_rwlockattr_t;

typedef union {
  unsigned char __librs_internal_size[4];
  int __librs_internal_align;
} pthread_rwlock_t;

typedef union {
  unsigned char __librs_internal_size[24];
  int __librs_internal_align;
} pthread_barrier_t;

typedef union {
  unsigned char __librs_internal_size[4];
  int __librs_internal_align;
} pthread_barrierattr_t;

typedef union {
  unsigned char __librs_internal_size[12];
  int __librs_internal_align;
} pthread_mutex_t;

typedef union {
  unsigned char __librs_internal_size[20];
  int __librs_internal_align;
} pthread_mutexattr_t;

typedef union {
  unsigned char __librs_internal_size[8];
  int __librs_internal_align;
} pthread_condattr_t;

typedef union {
  unsigned char __librs_internal_size[8];
  int __librs_internal_align;
} pthread_cond_t;

typedef union {
  unsigned char __librs_internal_size[4];
  int __librs_internal_align;
} pthread_spinlock_t;

typedef union {
  unsigned char __librs_internal_size[4];
  int __librs_internal_align;
} pthread_once_t;

typedef void *pthread_t;

typedef unsigned long pthread_key_t;

#endif  /* _LIBRS_BITS_PTHREAD_H */
