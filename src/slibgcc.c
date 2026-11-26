#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <string.h>
#include <signal.h>
#include <ctype.h>
#include "slib.h"
#include "pktls.h"
#ifndef NULL
#define NULL (void *)(0)
#endif
#ifndef EXIT_FAILURE
#define EXIT_FAILURE 1
#endif


/*   Copyright (C) 1989, California Institute of Technology */
/*   All rights reserved.  U. S. Government Sponsorship under */
/*   NASA Contract NAS7-918 is acknowledged. */

/*   Herbert M. Pickett, 20 Mar 1989 */
/*   Revised version in c, 22 March 1999 */

/* potentially system dependent functions */

/***************** run control for library use (pickett.h) *************/
/* pk_fail() replaces exit(EXIT_FAILURE): when a run function is active  */
/* it closes the files opened with fopenq() and longjmps back to it.     */
static PK_TLS /*@null@*/ jmp_buf *pk_jump = NULL;
static PK_TLS int pk_libmode = 0;
#define PK_MAXFILES 64
static PK_TLS /*@null@*/ FILE *pk_files[PK_MAXFILES];

void pk_library_mode(int on)
{
  pk_libmode = on;
}

int pk_thread_safe(void)
{
  return PK_THREAD_SAFE;
}

static PK_TLS int pk_nthreads = 0;   /* <= 0: one per CPU core */

void pk_set_threads(int n)
{
  pk_nthreads = n;
}

int pk_get_threads(void)
{ /* pk_set_threads(), else $PICKETT_THREADS, else one per core */
  const char *env;
  int n;
  if (pk_nthreads > 0) return pk_nthreads;
  env = getenv("PICKETT_THREADS");
  n = (env != NULL) ? atoi(env) : 0;
  return (n > 0) ? n : pk_cpu_count();
}

void pk_set_jump(jmp_buf *jb)
{
  pk_jump = jb;
}

static void pk_track(FILE *f)
{
  int k;
  for (k = 0; k < PK_MAXFILES; ++k) {
    if (pk_files[k] == NULL) {
      pk_files[k] = f;
      return;
    }
  }
}

static int pk_vfs_close(FILE *f);

int pk_fclose(FILE *f)
{ /* fclose() for library code; calpgm.h maps fclose to this */
  int k;
  for (k = 0; k < PK_MAXFILES; ++k) {
    if (pk_files[k] == f) pk_files[k] = NULL;
  }
  pk_vfs_close(f);   /* keeps the contents of a memory output file */
  return fclose(f);
}

void pk_close_all(void)
{ /* close files that fopenq() opened and nobody closed */
  int k;
  for (k = 0; k < PK_MAXFILES; ++k) {
    if (pk_files[k] != NULL) pk_fclose(pk_files[k]);
  }
}

void pk_fail(int code)
{
  if (code == 0) code = EXIT_FAILURE;
  fflush(pk_stdout());
  if (pk_jump != NULL) longjmp(*pk_jump, code);
  exit(code);
}

/***************** memory files (pk_vfs_*, see pickett.h) ****************/
typedef struct {
  char *name;
  char *data;        /* always NUL-terminated */
  size_t len;
  FILE *wfp;         /* temporary file while open for writing */
} PK_VFILE;
static PK_TLS PK_VFILE *pk_vf = NULL;
static PK_TLS int pk_nvf = 0, pk_maxvf = 0, pk_vfs_on = 0;

static int vf_find(const char *name)
{
  int k;
  for (k = 0; k < pk_nvf; ++k) {
    if (strcmp(pk_vf[k].name, name) == 0) return k;
  }
  return -1;
}

static int vf_add(const char *name)
{ /* index of a new, empty entry, or -1 */
  PK_VFILE *p;
  int n;
  if (pk_nvf == pk_maxvf) {
    n = pk_maxvf ? 2 * pk_maxvf : 16;
    p = (PK_VFILE *) realloc(pk_vf, (size_t) n * sizeof(PK_VFILE));
    if (p == NULL) return -1;
    pk_vf = p; pk_maxvf = n;
  }
  p = &pk_vf[pk_nvf];
  p->name = (char *) malloc(strlen(name) + 1);
  p->data = (char *) malloc(1);
  if (p->name == NULL || p->data == NULL) {
    free(p->name); free(p->data);
    return -1;
  }
  strcpy(p->name, name);
  p->data[0] = '\0'; p->len = 0; p->wfp = NULL;
  return pk_nvf++;
}

void pk_vfs_begin(void)
{
  pk_vfs_end();
  pk_vfs_on = 1;
}

void pk_vfs_end(void)
{
  int k;
  for (k = 0; k < pk_nvf; ++k) {
    if (pk_vf[k].wfp != NULL) fclose(pk_vf[k].wfp);
    free(pk_vf[k].name);
    free(pk_vf[k].data);
  }
  free(pk_vf);
  pk_vf = NULL;
  pk_nvf = pk_maxvf = 0;
  pk_vfs_on = 0;
}

int pk_vfs_put(const char *name, const char *data, size_t len)
{
  char *buf;
  size_t i, n;
  int k;
  k = vf_find(name);
  if (k < 0) k = vf_add(name);
  if (k < 0) return -1;
  buf = (char *) malloc(len + 1);
  if (buf == NULL) return -1;
  for (i = n = 0; i < len; ++i) {   /* "\r\n" -> "\n" */
    if (data[i] == '\r' && i + 1 < len && data[i + 1] == '\n') continue;
    buf[n++] = data[i];
  }
  buf[n] = '\0';
  free(pk_vf[k].data);
  pk_vf[k].data = buf;
  pk_vf[k].len = n;
  return 0;
}

int pk_vfs_get(const char *name, const char **data, size_t *len)
{
  int k = vf_find(name);
  if (k < 0) return -1;
  *data = pk_vf[k].data;
  *len = pk_vf[k].len;
  return 0;
}

int pk_vfs_count(void)
{
  return pk_nvf;
}

const char *pk_vfs_name(int k)
{
  return (k >= 0 && k < pk_nvf) ? pk_vf[k].name : NULL;
}

FILE *pk_fopen(const char *name, const char *mode)
{ /* fopen(), or a memory file between pk_vfs_begin() and pk_vfs_end() */
  FILE *f;
  int k;
  if (!pk_vfs_on) return fopen(name, mode);
  k = vf_find(name);
  if (mode[0] == 'r') {
    if (k < 0 || pk_vf[k].wfp != NULL) return NULL;
    f = pk_tmpfile();
    if (f == NULL) return NULL;
    if (pk_vf[k].len > 0) fwrite(pk_vf[k].data, 1, pk_vf[k].len, f);
    rewind(f);
    return f;
  }
  if (k < 0) k = vf_add(name);
  if (k < 0 || pk_vf[k].wfp != NULL) return NULL;
  f = pk_tmpfile();
  if (f == NULL) return NULL;
  if (mode[0] == 'a' && pk_vf[k].len > 0) fwrite(pk_vf[k].data, 1, pk_vf[k].len, f);
  pk_vf[k].wfp = f;
  return f;
}

static int pk_vfs_close(FILE *f)
{ /* if f is a memory output file, keep its contents; 1 if it was */
  char *buf;
  long n;
  int k;
  for (k = 0; k < pk_nvf; ++k) {
    if (pk_vf[k].wfp != f) continue;
    pk_vf[k].wfp = NULL;
    fflush(f);
    fseek(f, 0L, SEEK_END);
    n = ftell(f);
    if (n < 0) n = 0;
    buf = (char *) malloc((size_t) n + 1);
    if (buf == NULL) return 1;
    rewind(f);
    n = (long) fread(buf, 1, (size_t) n, f);
    buf[n] = '\0';
    free(pk_vf[k].data);
    pk_vf[k].data = buf;
    pk_vf[k].len = (size_t) n;
    return 1;
  }
  return 0;
}

/***************** program messages (pk_log_*, see pickett.h) ************/
static PK_TLS int pk_logmode = PK_LOG_STDOUT;
static PK_TLS /*@null@*/ FILE *pk_logfp = NULL;
static PK_TLS /*@null@*/ char *pk_logbuf = NULL;

FILE *pk_stdout(void)
{
  if (pk_logmode != PK_LOG_STDOUT) {
    if (pk_logfp == NULL) pk_logfp = pk_tmpfile();
    if (pk_logfp != NULL) return pk_logfp;
  }
  return stdout;
}

int pk_puts(const char *s)
{
  FILE *f = pk_stdout();
  fputs(s, f);
  return fputc('\n', f);
}

void pk_log_mode(int mode)
{
  if (pk_logfp != NULL) {
    fclose(pk_logfp);
    pk_logfp = NULL;
  }
  free(pk_logbuf);
  pk_logbuf = NULL;
  pk_logmode = mode;
}

const char *pk_log_text(size_t *len)
{
  long n = 0;
  free(pk_logbuf);
  pk_logbuf = NULL;
  if (pk_logmode == PK_LOG_CAPTURE && pk_logfp != NULL) {
    fflush(pk_logfp);
    fseek(pk_logfp, 0L, SEEK_END);
    n = ftell(pk_logfp);
    if (n < 0) n = 0;
    pk_logbuf = (char *) malloc((size_t) n + 1);
    if (pk_logbuf != NULL) {
      rewind(pk_logfp);
      n = (long) fread(pk_logbuf, 1, (size_t) n, pk_logfp);
      pk_logbuf[n] = '\0';
      fseek(pk_logfp, 0L, SEEK_END);
    } else {
      n = 0;
    }
  }
  if (len != NULL) *len = (size_t) n;
  return pk_logbuf != NULL ? pk_logbuf : "";
}

/***************** result callbacks (pk_set_sink, see pickett.h) *********/
static PK_TLS /*@null@*/ const PK_SINK *pk_sink = NULL;

void pk_set_sink(const PK_SINK *sink)
{
  pk_sink = sink;
}

const PK_SINK *pk_get_sink(void)
{
  return pk_sink;
}

int chtime(str, N)
char str[];
const int N;
{ /* subroutine to return 24 characters with the time and date */
#define NCTIME 24
  char *buffer;
  time_t curtime;
  struct tm *loctime;
  int k, n;
#ifndef _WIN32
  struct tm tmbuf;
  char abuf[32];
#endif
  n = N;
  /* Get the current time. */
  curtime = time (NULL);
  /* Convert it to local time representation. The Microsoft C runtime */
  /* keeps per-thread buffers; elsewhere use the reentrant versions.  */
#ifdef _WIN32
  loctime = localtime (&curtime);
  buffer = (loctime != NULL) ? asctime (loctime) : NULL;
#else
  loctime = localtime_r (&curtime, &tmbuf);
  buffer = (loctime != NULL) ? asctime_r (loctime, abuf) : NULL;
#endif
  /* copy the date and time in the standard format. */
  if (buffer != NULL) {
    k= (int) strlen(str);
    n -= NCTIME + 2;
    while(k < n) str[k++]=' ';
    str += n;
    memcpy(str,buffer, (size_t) 24);
    str[NCTIME] = '\n'; str[NCTIME + 1] = '\0';
  }
  return 0;
} /* chtime */

int caldelay(delay)
int delay;
{ 
  /* if delay is different, save time and return false */  
  /* else return true if time interval >= delay */
  static PK_TLS time_t tcmp = 0;
  static PK_TLS int old_delay = -1;

  if (delay == old_delay) {
    if (time(NULL) >= tcmp) {
      tcmp += delay; return 1;
    }
  } else {
    tcmp = time(NULL) + delay; old_delay = delay;
    if (delay <= 0) return 1;
  }
  return 0;
} /* caldelay */

void *mallocq(size_t nl)
{
  void *p;
  p = malloc((size_t) nl);
  if (p == NULL) {
    pk_puts("memory allocation error");
    pk_fail(EXIT_FAILURE);
  } 
  return p;
}
int maxmem(size_t *nll)
{
  /* On return 'nll' is the maximum double precision size that can be */ 
  /* allocated at once. This function returns the largest dimension of */
  /* a square double-precision matrix that can be allocated */
  size_t nl, nn, nroot;
  int k, n;
  n = (int) sizeof(size_t);
  nl = (size_t) 0x100 / sizeof(double) - 1;
  for (k = 1; k < n; ++k) nl = (nl << 8) + 0x0ff;
  if (nll) *nll = nl;
  nroot =  90; 
  for(k = 2; k < n; k += 2) nroot <<= 8; 
  do { /* iterative square root */
    nn = nroot; nroot = (nl /(nn + 1) + nn) >> 1;
  } while (nn != nroot);
  n = (int) sizeof(int);
  nl = 0x7fff;
  for(k = 2; k < n; ++k)  nl = (nl << 8) + 0x0ff;
  if (nroot > nl) nroot = nl;   
  return (int)nroot;
}
int filget(argc, argv,  nfile, cfil, cext)
const int   argc, nfile;
char *argv[];
char *cfil[];
const char *cext[];
{
#define NSTR 82
  char str[NSTR];
  char *master, *ext, *parg, *opt;
  /*@ dependent @*/ char *pstr;
  int  k, iext, len, iarg, knt, query;
  /*     Get NFILE names with extent given in CEXT */
  /*     Put names in CFIL */

  /*     gets file names from command line or interactively */
  /*     ARGC+1 is the number of command line arguments */
  /*     ARGV(I) is the I-th command line argument */

  /*     define character which delimits EXT field */

  for (iext=0; iext <= nfile; ++iext) cfil[iext]=NULL;
  knt = argc - 1; query = 0;
  if (knt <= 0) {
    query = 1; knt = nfile + 1;
  }
  master = NULL; opt = NULL; len = 0;
  for (iarg = 1; iarg <= knt; ++iarg) {
    if (query != 0) {
      parg = str;
      pk_puts(" Enter file name ");
      if (fgetstr(parg, NSTR, stdin) <= 0) break;
    } else {
      parg = argv[iarg];
      if (parg == NULL) continue;
    }
    ext = strrchr(parg,'.');
    if (ext != NULL) {/* dot found */
      ++ext;
      for(iext = 0; iext < nfile; ++iext) {
        if (strcmp(cext[iext], ext) == 0) {
          k = (int) strlen(parg) + 1;
          pstr = (char *)malloc((size_t) k);
          if (pstr != NULL) memcpy(pstr, parg, (size_t)k);
          cfil[iext] = pstr;
          break;
        }
      }
    }
    if (parg[0] == '-') { /* set up options */
      if (opt != NULL) continue;
      k = (int) strlen(parg);
      opt = (char *)malloc((size_t) k);
      if (opt == NULL) break;
      --k;
      memcpy(opt, parg + 1, (size_t) k);
      opt[k] = '\0'; cfil[nfile] = opt;
      continue;
    }
    if (master == NULL) { /* set up default file name */
      k = (int) strlen(parg) + 1;
      if (ext != NULL) k -= (int)strlen(ext) + 1;
      if (k <= 0) continue;
      master = (char *)malloc((size_t) k);
      if (master == NULL) break; 
      len = k; --k;
      memcpy(master, parg, (size_t) k);
      master[k] = '.';
    }
  }
  if (master == NULL) return 0;
  for (iext = 0; iext < nfile; ++iext) {/* find default values */
    pstr = cfil[iext];
    if(pstr == NULL) {
      k = (int)strlen(cext[iext]) + 1;
      pstr = (char *) malloc((size_t)(len + k));
      if(pstr != NULL) { 
        memcpy(pstr, master, (size_t) len);
        memcpy(pstr + len, cext[iext], (size_t) k);
        cfil[iext] = pstr;
      }
    }
    if (pstr != NULL) pk_puts(pstr);
  }
  free (master);
  return nfile;
} /* filget */

FILE *fopenq(const char *fname, const char *opt)
{
  FILE *hret;
  char *fnew, *path;
  if(opt[0] == 'l') {
    hret = pk_fopen(fname, "r");
    if (hret == NULL && pk_vfs_on) hret = fopen(fname, "r"); /* labels from disk */
    if(hret == NULL) {
      path = getenv("SPECNAME");
      if (path != NULL){
        fnew = (char *)malloc(strlen(fname) + strlen(path) + 1);
        if (fnew != NULL) {
          strcpy(fnew,path); strcat(fnew,fname);
          hret = fopen(fnew,"r");
          free(fnew);
        }
      }else {
        pk_puts("path variable SPECNAME can be set to path for names");
      }
    }
  } else {
    hret = pk_fopen(fname,opt);
    if(hret == NULL) {
      fprintf(pk_stdout(), " Trouble opening %s\n", fname);
      pk_fail(EXIT_FAILURE);
    }
  }
  if (hret != NULL) pk_track(hret);
  return hret;
} /* fopenq */

int fgetstr(buffer, N, stream)
char *buffer;
const int N;
FILE *stream;
{ /* read first n bytes of line */
  char *pbuf;
  int k, n;
  n = N;
  if (stream != NULL && fgets(buffer,n,stream) == NULL) {
    buffer[0] = '\0';
    n = -1;
  } else {
    k = n = 0;
    for (pbuf = buffer; *pbuf != '\0' && *pbuf != '\n'; ++pbuf) {
      ++k;
      if (*pbuf != ' ' && !iscntrl(*pbuf)) n = k;
    }
    if(*pbuf == '\0') {
      do {
        k = fgetc(stream);
      } while (k != (int)'\n' && k != EOF); /* flush rest of line */
    }
    buffer[n] = '\0';
  }
  return n;
} /* fgetstr */

static PK_TLS int brkflg;

int rqexit(ival)
const int ival;
{
  if (ival != 0) {
    if (pk_libmode == 0) (void)signal(SIGINT, brkqr);
    brkflg = 0;
  }
  return brkflg;
} /* rqexit */

void brkqr(int i)
{
    if(i != 0) brkflg = 1;
} /* brkqr */













