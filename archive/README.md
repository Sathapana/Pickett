# Archive

Files from the original 2008 distribution that nothing builds, includes or tests any more. They are kept here, with their git history (`git log --follow archive/<path>`), for reference. They are **not** part of the build:
- the [Makefile](../Makefile) does not compile them;
- the Rust build (`rust/pickett-sys/build.rs`) does not compile them;
- no remaining source file includes them;
- `test/run_golden.py` does not use them.

Before they were moved, each file was checked against all of the above. To use one again, move it back to `src/` and add it to the Makefile.

## old-versions/: superseded copies of current code

| File | What it is | Current version |
|---|---|---|
| `spinv0.c` | Older SPINV Hamiltonian; last revision 12 Dec 2005 | `spinv.c` (adds the 10 Sept 2006 phase conventions and later changes) |
| `calfitno.c` | Older SPFIT driver with pre-2001 iteration control | `calfit.c` |
| `dpix.c` | Variant of the doublet-Π Hamiltonian, with a different matrix-element pair table | `dpi.c` |
| `slibgen.c` | Generic, non-gcc version of the system library | `slibgcc.c` |
| `blas.c`, `blas.h` | Pre-CBLAS subset of BLAS (K&R) | `dblas.c` / `cblas.h` |
| `jelim.c` | Old stand-alone `jelim()` (4 arguments) | `jelim()` in `lsqfit.c` (5 arguments) |
| `fdate.c` | `strdate_()`, a date function to be called from Fortran; nothing calls it | `chtime()` in `slibgcc.c` |
| `testi.c` | Prints the sizes of C types | — |

## jpl-catalog-tools/: tools for the JPL catalog directory

| File | What it is |
|---|---|
| `catread.c`, `catread.h` | Reads the JPL catalog directory and its species files |
| `catlist.c` | Lists catalog lines between two frequencies (interactive; uses `gets()`) |
| `catform.c` | CGI handler of the JPL catalog web form |
| `util.c` | CGI helpers (`makeword`, `unescape_url`, …), used only by `catform.c` |

These are old K&R C, and `catlist.c` uses `gets()`, which modern C no longer has. `catread.c` includes `catutil.h` from `src/`. No makefile ever built them. To read and write catalog files today, use `rust/pickett-io` (`CatFile`).

## build/: obsolete makefiles

| File | What it is |
|---|---|
| `makefile.linux` | g77-era makefile. It refers to Fortran sources that are not in the repository (`calbak.f`, `cnvwn.f`, `slib.for`) |
| `Makefile.txt` | Older alternate makefile. The main `Makefile` now covers all its targets |
