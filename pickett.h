/*  pickett.h -- library entry points of the CALPGM suite                  */
/*                                                                          */
/*  calcat_run() is the body of SPCAT (linked with spinv.c + spinit.c) or   */
/*  DPCAT (linked with dpi.c); calfit_run() is the body of SPFIT or DPFIT.  */
/*  Both take the same arguments as the command-line programs, for example  */
/*  { "spcat", "mol" } or explicit file names such as "dir/mol.int".        */
/*                                                                          */
/*  They return 0 on success and a non-zero status on a fatal error. A      */
/*  fatal error no longer ends the process: pk_fail() jumps back to the     */
/*  run function, which closes the files it opened and returns the status.  */
/*  The error message is still printed on stdout.                           */
/*                                                                          */
/*  Not thread-safe: the code keeps state in static variables, so only one  */
/*  run may be active at a time in a process. After a failure in the middle */
/*  of a calculation, allocated memory is not freed.                        */
#ifndef PICKETT_H
#define PICKETT_H
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

int calcat_run(int argc, char *argv[]);
int calfit_run(int argc, char *argv[]);

/* on != 0: do not install the SIGINT (Ctrl-C) handler that the programs  */
/* use to stop early, so the host application keeps its own handler.      */
void pk_library_mode(int on);

/* 1 when each thread has its own copy of the library state (see pktls.h): */
/* then different threads may run programs at the same time.               */
int pk_thread_safe(void);

/* Fatal error inside the library: return `code` from the active run       */
/* function, or exit(code) when no run function is active.                 */
void pk_fail(int code);

/***************************** memory files ******************************/
/* Between pk_vfs_begin() and pk_vfs_end(), the programs read and write    */
/* named buffers instead of files: put the inputs (for example "mol.int"   */
/* and "mol.var"), run with base name "mol", then get the outputs. Open    */
/* streams use anonymous temporary files, so no named file is created.     */
/* "\r\n" in put data becomes "\n". Parameter-label files (*.nam) that are */
/* not put are still looked up on disk.                                    */
void pk_vfs_begin(void);
void pk_vfs_end(void);                      /* frees every buffer */
int  pk_vfs_put(const char *name, const char *data, size_t len); /* 0: ok */
/* data stays valid until the next put/run/end; returns 0 when found */
int  pk_vfs_get(const char *name, const char **data, size_t *len);
int  pk_vfs_count(void);
const char *pk_vfs_name(int k);             /* NULL when k is out of range */

/***************************** program messages **************************/
#define PK_LOG_STDOUT  0   /* print on stdout (default) */
#define PK_LOG_CAPTURE 1   /* keep the text; read it with pk_log_text() */
#define PK_LOG_DISCARD 2   /* drop it */
void pk_log_mode(int mode);
/* text captured since pk_log_mode(PK_LOG_CAPTURE); valid until the next  */
/* call of pk_log_text() or pk_log_mode()                                  */
const char *pk_log_text(size_t *len);

/***************************** result callbacks **************************/
/* Called with full-precision values while a run produces its results.    */
/* Any member may be NULL. Units: frequencies in MHz (cm-1 in IR mode),   */
/* energies in cm-1, lgint = log10 intensity (nm^2 MHz), str = S*mu^2.    */
typedef struct pk_sink {
  void *user;
  /* SPCAT/DPCAT: every catalog line, in calculation order (the .cat file */
  /* is sorted by frequency afterwards); nqn quanta per state             */
  void (*cat_line)(void *user, double freq, double err, double lgint,
                   double str, int dr, double elow, int gup, long tag,
                   int qnfmt, int nqn, const short *upper, const short *lower);
  /* SPCAT/DPCAT: every energy level used (whether or not .egy is written) */
  /* ktsp is the "(nnn)" field of .egy, or -1 when there is none           */
  void (*level)(void *user, int block, int index, double energy, double err,
                double mixing, int degeneracy, int ktsp, int nqn,
                const short *qn);
  /* SPCAT/DPCAT: partition-function table, one call per temperature     */
  void (*partition)(void *user, double temp, double q);
  /* SPFIT/DPFIT: end of each iteration (the numbers printed in .fit)     */
  void (*fit_iteration)(void *user, int iteration, double rms_error_old,
                        double rms_error_new, double rms_mw, double rms_ir,
                        double avg_mw, double avg_ir, int nlines_used,
                        int nlines_rejected);
  /* SPFIT/DPFIT: each parameter as written to .par/.var; err_fit is the  */
  /* fitted error (.var), err_par the error written to .par              */
  void (*fit_param)(void *user, int index, const char *id, double value,
                    double err_fit, double err_par, const char *label);
} PK_SINK;
void pk_set_sink(const PK_SINK *sink);  /* NULL removes it */

#ifdef __cplusplus
}
#endif

#endif /* PICKETT_H */
