/* Generated contract — keep in sync with userland/abi/syscall_numbers.json */
#ifndef XDOS_SYSCALL_H
#define XDOS_SYSCALL_H

#include <stddef.h>
#include <stdint.h>

#define XDOS_SYS_write   1
#define XDOS_SYS_read    2
#define XDOS_SYS_yield   3
#define XDOS_SYS_getpid  39
#define XDOS_SYS_exit    60

#define XDOS_ENOSYS 38
#define XDOS_EINVAL 22
#define XDOS_EEFAULT 14

#define XDOS_FD_SERIAL 1
#define XDOS_FD_VGA    2

static inline int64_t xdos_syscall6(
    uint64_t num,
    uint64_t a1,
    uint64_t a2,
    uint64_t a3,
    uint64_t a4,
    uint64_t a5,
    uint64_t a6
) {
    int64_t ret;
    __asm__ volatile(
        "syscall"
        : "=a"(ret)
        : "a"(num), "D"(a1), "S"(a2), "d"(a3), "r"(a4), "r"(a5), "r"(a6)
        : "rcx", "r11", "memory");
    return ret;
}

static inline int64_t xdos_write(int fd, const void *buf, size_t len) {
    return xdos_syscall6(XDOS_SYS_write, (uint64_t)fd, (uint64_t)(uintptr_t)buf, (uint64_t)len, 0, 0, 0);
}

static inline int64_t xdos_read(void) {
    return xdos_syscall6(XDOS_SYS_read, 0, 0, 0, 0, 0, 0);
}

static inline int64_t xdos_yield(void) {
    return xdos_syscall6(XDOS_SYS_yield, 0, 0, 0, 0, 0, 0);
}

static inline int64_t xdos_getpid(void) {
    return xdos_syscall6(XDOS_SYS_getpid, 0, 0, 0, 0, 0, 0);
}

static inline void xdos_exit(int code) {
    (void)xdos_syscall6(XDOS_SYS_exit, (uint64_t)(uint32_t)code, 0, 0, 0, 0, 0);
    for (;;) {
        __asm__ volatile("hlt");
    }
}

#endif
