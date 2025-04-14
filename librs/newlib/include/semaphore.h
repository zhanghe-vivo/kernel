#ifndef _LIBRS_SEMAPHORE_H
#define _LIBRS_SEMAPHORE_H

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/semaphore.h.html>.
 */
typedef union {
  char size[4];
  long align;
} sem_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_destroy.html>.
 */
int sem_destroy(sem_t *sem);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_getvalue.html>.
 */
int sem_getvalue(sem_t *sem, int *sval);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_init.html>.
 */
int sem_init(sem_t *sem, int _pshared, unsigned int value);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_post.html>.
 */
int sem_post(sem_t *sem);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_trywait.html>.
 */
int sem_trywait(sem_t *sem);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/sem_trywait.html>.
 */
int sem_wait(sem_t *sem);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_SEMAPHORE_H */
