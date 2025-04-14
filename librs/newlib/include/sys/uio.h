#ifndef _SYS_UIO_H
#define _SYS_UIO_H

#include <sys/types.h>

#define IOV_MAX 1024

struct iovec {
  void *iov_base;
  size_t iov_len;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

ssize_t readv(int fd, const struct iovec *iov, int iovcnt);

ssize_t writev(int fd, const struct iovec *iov, int iovcnt);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _SYS_UIO_H */
