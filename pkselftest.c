/* pkselftest.c -- checks that the CALPGM library can run in-process.      */
/*                                                                          */
/* usage: pkselftest cat|fit ARG... -- INPUT...                             */
/*                                                                          */
/* In the current directory it                                              */
/*   1. calls the run function on a missing file and checks that it        */
/*      returns a failure status instead of ending the process,             */
/*   2. calls calcat_run (cat) or calfit_run (fit) with ARG..., restores    */
/*      the INPUT files, and calls it again in the same process.            */
/* The outputs left behind are those of the second run, so comparing them   */
/* with the golden files shows whether state leaks between runs.            */
/* Linked with spinv.c it tests SPCAT/SPFIT, with dpi.c DPCAT/DPFIT.        */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "pickett.h"

typedef struct {
  const char *name;
  char *data;
  size_t len;
} SNAP;

static int run(int fit, int argc, char *argv[])
{
  return fit ? calfit_run(argc, argv) : calcat_run(argc, argv);
}

static int snap_read(SNAP *s)
{
  FILE *f = fopen(s->name, "rb");
  long n;
  if (f == NULL) return -1;
  fseek(f, 0L, SEEK_END);
  n = ftell(f);
  rewind(f);
  s->data = (char *) malloc((size_t) n + 1);
  if (s->data == NULL) { fclose(f); return -1; }
  s->len = fread(s->data, 1, (size_t) n, f);
  fclose(f);
  return 0;
}

static int snap_write(const SNAP *s)
{
  FILE *f = fopen(s->name, "wb");
  if (f == NULL) return -1;
  fwrite(s->data, 1, s->len, f);
  fclose(f);
  return 0;
}

int main(int argc, char *argv[])
{
  static const char *junk[] = { "out", "cat", "str", "egy", "fit", "bak",
                                "var", "par", NULL };
  static char missing[] = "pk_selftest_missing";
  char *bad[2];
  char name[64];
  char **rargv;
  SNAP *snap;
  int fit, sep, nargs, ninp, k, pass, rc;

  if (argc < 3 || (strcmp(argv[1], "cat") != 0 && strcmp(argv[1], "fit") != 0)) {
    puts("usage: pkselftest cat|fit ARG... -- INPUT...");
    return 2;
  }
  fit = (strcmp(argv[1], "fit") == 0);
  for (sep = 2; sep < argc && strcmp(argv[sep], "--") != 0; ++sep);
  nargs = sep - 2;
  ninp = (sep < argc) ? argc - sep - 1 : 0;
  pk_library_mode(1);

  /* 1. a fatal error must come back as a status, not end the process */
  bad[0] = argv[0];
  bad[1] = missing;
  rc = run(fit, 2, bad);
  for (k = 0; junk[k] != NULL; ++k) {
    sprintf(name, "%s.%s", missing, junk[k]);
    remove(name);
  }
  if (rc == 0) {
    puts("pkselftest: run on a missing file did not fail");
    return 3;
  }
  printf("pkselftest: failure returned status %d as expected\n", rc);

  /* 2. two successful runs in the same process */
  snap = (SNAP *) calloc((size_t) ninp + 1, sizeof(SNAP));
  rargv = (char **) calloc((size_t) nargs + 2, sizeof(char *));
  if (snap == NULL || rargv == NULL) return 4;
  for (k = 0; k < ninp; ++k) {
    snap[k].name = argv[sep + 1 + k];
    if (snap_read(&snap[k]) != 0) {
      printf("pkselftest: cannot read input %s\n", snap[k].name);
      return 4;
    }
  }
  rargv[0] = argv[0];
  for (k = 0; k < nargs; ++k) rargv[k + 1] = argv[2 + k];
  for (pass = 1; pass <= 2; ++pass) {
    for (k = 0; k < ninp; ++k) snap_write(&snap[k]);
    rc = run(fit, nargs + 1, rargv);
    printf("pkselftest: run %d returned %d\n", pass, rc);
    if (rc != 0) return rc;
  }
  return 0;
}
