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

#ifdef __cplusplus
extern "C" {
#endif

int calcat_run(int argc, char *argv[]);
int calfit_run(int argc, char *argv[]);

/* on != 0: do not install the SIGINT (Ctrl-C) handler that the programs  */
/* use to stop early, so the host application keeps its own handler.      */
void pk_library_mode(int on);

/* Fatal error inside the library: return `code` from the active run       */
/* function, or exit(code) when no run function is active.                 */
void pk_fail(int code);

#ifdef __cplusplus
}
#endif

#endif /* PICKETT_H */
