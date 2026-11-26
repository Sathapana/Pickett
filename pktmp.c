/* pktmp.c -- anonymous temporary files that work on every platform.       */
/*                                                                          */
/* On Windows the C runtime's tmpfile() creates its file in the root of the */
/* current drive (for example C:\), which ordinary users may not write to,  */
/* so it can return NULL. pk_tmpfile() uses the user's temporary folder     */
/* instead and opens the file with "D" so it is deleted when closed. On     */
/* other systems it is tmpfile(). calpgm.h maps tmpfile() to this.          */
/* Kept in its own file so windows.h does not meet the old code's BOOL.     */
#include <stdio.h>

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
