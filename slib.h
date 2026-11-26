#ifndef FILE
#include <stdio.h>
#endif
/************** SLIB interface ***************************************** */
int  chtime(char str[], const int n);
int  caldelay(int delay);
/*@out@*/ /*@only@*/ /*@notnull@*/ void *mallocq(size_t nl);
int  maxmem(/*@null@*/ /*@out@*/ size_t *nl);
int  filget(const int argc, char *argv[], const int nfile,
            /*@out@*/ char *cfil[], const char *cext[]);
/*@dependent@*/ FILE *fopenq(const char *fname, const char *opt)
                             /*@modifies fileSystem@*/ ;
int  fgetstr(/*@out@*/char buffer[], const int n, FILE *stream);
int  rqexit(const int ival);
void brkqr(int);
/************** run control for library use (see pickett.h) ************ */
#include <setjmp.h>
#include "pickett.h"
void pk_set_jump(/*@null@*/ jmp_buf *jb);
int  pk_fclose(FILE *f);
void pk_close_all(void);
FILE *pk_fopen(const char *name, const char *mode); /* memory files aware */
FILE *pk_stdout(void);            /* where program messages go */
FILE *pk_tmpfile(void);           /* tmpfile() that works on Windows (pktmp.c) */
int  pk_puts(const char *s);
/*@null@*/ const PK_SINK *pk_get_sink(void);
