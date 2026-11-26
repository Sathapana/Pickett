/* pktls.h -- per-thread state for the CALPGM library                      */
/*                                                                          */
/* The programs keep their working state in static variables. Declaring    */
/* those `static PK_TLS` gives every thread its own copy, so different     */
/* threads can run SPCAT/SPFIT/DPCAT/DPFIT at the same time. Read-only      */
/* tables stay plain `static` (shared).                                     */
/*                                                                          */
/* Define PK_NO_TLS to build without thread-local storage; the library is  */
/* then for one run at a time, and pk_thread_safe() returns 0.              */
#ifndef PKTLS_H
#define PKTLS_H

#if defined(PK_NO_TLS)
#define PK_TLS
#define PK_THREAD_SAFE 0
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L && !defined(__STDC_NO_THREADS__) && !defined(_MSC_VER)
#define PK_TLS _Thread_local
#define PK_THREAD_SAFE 1
#elif defined(__GNUC__)
#define PK_TLS __thread
#define PK_THREAD_SAFE 1
#elif defined(_MSC_VER)
#define PK_TLS __declspec(thread)
#define PK_THREAD_SAFE 1
#else
#define PK_TLS
#define PK_THREAD_SAFE 0
#endif

#endif /* PKTLS_H */
