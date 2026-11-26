/* pktmp.c -- anonymous temporary files that work on every platform.       */
/*                                                                          */
/* On Windows the C runtime's tmpfile() creates its file in the root of the */
/* current drive (for example C:\), which ordinary users may not write to,  */
/* so it can return NULL. pk_tmpfile() uses the user's temporary folder     */
/* instead and opens the file with "D" so it is deleted when closed. On     */
/* other systems it is tmpfile(). calpgm.h maps tmpfile() to this.          */
/* Kept in its own file so windows.h does not meet the old code's BOOL.     */
/*                                                                          */
/* pk_fpu_enter()/pk_fpu_leave(): the math library (exp, pow, log10, ...)   */
/* may use x87 instructions, whose precision depends on the x87 control     */
/* word. The C runtime sets extended precision for the main thread, but    */
/* threads created later (e.g. by a host program) start at 53-bit          */
/* precision on Windows, which changes the last digits of the results.     */
/* The run functions set the runtime's default (0x037F: extended precision, */
/* round to nearest, exceptions masked) and restore the caller's value.    */
#include <stdio.h>
#include <stdlib.h>
#ifdef _WIN32
#include <windows.h>
#endif

/* pk_run_parallel(fn, ctx, n): calls fn(ctx, part, n) for part = 0..n-1,  */
/* part 0 on the calling thread and the others on new threads, and waits    */
/* for all of them. Each worker uses the same x87 setting as a run          */
/* (pk_fpu_enter), so results do not depend on the thread. If a thread      */
/* cannot be created, its part runs on the calling thread.                  */
typedef void (*pk_task_fn)(void *ctx, int part, int nparts);
unsigned int pk_fpu_enter(void);
void pk_fpu_leave(unsigned int old);

typedef struct {
  pk_task_fn fn;
  void *ctx;
  int part, nparts;
} PK_TASK;

static void pk_task_body(PK_TASK *t)
{
  unsigned int fpu = pk_fpu_enter();
  t->fn(t->ctx, t->part, t->nparts);
  pk_fpu_leave(fpu);
}

#define PK_MAXTHREADS 64

#ifdef _WIN32
static DWORD WINAPI pk_task_start(LPVOID arg)
{
  pk_task_body((PK_TASK *) arg);
  return 0;
}

int pk_cpu_count(void)
{
  SYSTEM_INFO si;
  GetSystemInfo(&si);
  return si.dwNumberOfProcessors > 0 ? (int) si.dwNumberOfProcessors : 1;
}

void pk_run_parallel(pk_task_fn fn, void *ctx, int nparts)
{
  PK_TASK task[PK_MAXTHREADS];
  HANDLE h[PK_MAXTHREADS];
  int k, nh = 0;
  if (nparts > PK_MAXTHREADS) nparts = PK_MAXTHREADS;
  if (nparts < 1) nparts = 1;
  for (k = 0; k < nparts; ++k) {
    task[k].fn = fn; task[k].ctx = ctx; task[k].part = k; task[k].nparts = nparts;
  }
  for (k = 1; k < nparts; ++k) {
    h[nh] = CreateThread(NULL, 0, pk_task_start, &task[k], 0, NULL);
    if (h[nh] != NULL) {
      ++nh;
    } else {
      pk_task_body(&task[k]);   /* no thread: do it here */
    }
  }
  fn(ctx, 0, nparts);           /* the caller already has the run's setting */
  if (nh > 0) WaitForMultipleObjects((DWORD) nh, h, TRUE, INFINITE);
  for (k = 0; k < nh; ++k) CloseHandle(h[k]);
}

#else
#include <pthread.h>
#include <unistd.h>

static void *pk_task_start(void *arg)
{
  pk_task_body((PK_TASK *) arg);
  return NULL;
}

int pk_cpu_count(void)
{
  long n = sysconf(_SC_NPROCESSORS_ONLN);
  return n > 0 ? (int) n : 1;
}

void pk_run_parallel(pk_task_fn fn, void *ctx, int nparts)
{
  PK_TASK task[PK_MAXTHREADS];
  pthread_t th[PK_MAXTHREADS];
  int started[PK_MAXTHREADS];
  int k;
  if (nparts > PK_MAXTHREADS) nparts = PK_MAXTHREADS;
  if (nparts < 1) nparts = 1;
  for (k = 0; k < nparts; ++k) {
    task[k].fn = fn; task[k].ctx = ctx; task[k].part = k; task[k].nparts = nparts;
    started[k] = 0;
  }
  for (k = 1; k < nparts; ++k) {
    started[k] = (pthread_create(&th[k], NULL, pk_task_start, &task[k]) == 0);
    if (!started[k]) pk_task_body(&task[k]);
  }
  fn(ctx, 0, nparts);
  for (k = 1; k < nparts; ++k) {
    if (started[k]) pthread_join(th[k], NULL);
  }
}
#endif

#if defined(__GNUC__) && (defined(__x86_64__) || defined(__i386__))
unsigned int pk_fpu_enter(void)
{
  unsigned short old, cw = 0x037F;
  __asm__ __volatile__ ("fnstcw %0" : "=m" (old));
  __asm__ __volatile__ ("fldcw %0" : : "m" (cw));
  return old;
}

void pk_fpu_leave(unsigned int old)
{
  unsigned short cw = (unsigned short) old;
  __asm__ __volatile__ ("fldcw %0" : : "m" (cw));
}
#else
unsigned int pk_fpu_enter(void)
{
  return 0;
}

void pk_fpu_leave(unsigned int old)
{
  (void) old;
}
#endif

#ifdef _WIN32
#include <windows.h>

FILE *pk_tmpfile(void)
{
  char dir[MAX_PATH + 1], name[MAX_PATH + 1];
  FILE *f;
  DWORD n;
  n = GetTempPathA((DWORD) sizeof(dir), dir);
  if (n == 0 || n > (DWORD) sizeof(dir)) return tmpfile();
  if (GetTempFileNameA(dir, "pk", 0, name) == 0) return tmpfile();
  f = fopen(name, "w+bD");   /* D: delete the file when it is closed */
  if (f == NULL) {
    remove(name);
    return tmpfile();
  }
  return f;
}

#else

FILE *pk_tmpfile(void)
{
  return tmpfile();
}

#endif
