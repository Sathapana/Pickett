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
