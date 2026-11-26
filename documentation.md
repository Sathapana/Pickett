# Pickett CALPGM Suite: Developer & User Documentation

This repository holds the C source for **Herbert M. Pickett's CALPGM program suite**, best known for **SPFIT** (fitting) and **SPCAT** (prediction/catalog generation). Spectroscopists use these programs to fit and simulate rotational, ro-vibrational and hyperfine spectra of molecules. The programs come from JPL, and the CDMS (Cologne) and JPL molecular spectroscopy catalogs are built with them.

This document covers how the code is organised, how to build and run it, the main file formats, and what this fork changes. The authoritative physics and input-format reference is still the set of PDF manuals that ship with the code (see [References](#references)).

A user manual in Thai, with a step-by-step quick start and troubleshooting, is in [MANUAL_TH.md](MANUAL_TH.md).

---

## Contents

1. [Provenance](#provenance)
2. [Repository layout](#repository-layout)
3. [Architecture](#architecture)
4. [Building](#building)
5. [Running the programs](#running-the-programs)
6. [File formats (summary)](#file-formats-summary)
7. [Fork modification: extended partition function](#fork-modification-extended-partition-function)
8. [Test case and verification](#test-case-and-verification)
9. [Auxiliary programs](#auxiliary-programs)
10. [Rust file library: pickett-io](#rust-file-library-pickett-io)
11. [Rust in-process runner: pickett-sys](#rust-in-process-runner-pickett-sys)
12. [Rust high-level API: pickett](#rust-high-level-api-pickett)
13. [Rust spectrum analysis: pickett-spectrum](#rust-spectrum-analysis-pickett-spectrum)
14. [Legacy, alternate and unused files](#legacy-alternate-and-unused-files)
15. [Known issues and caveats](#known-issues-and-caveats)
16. [References](#references)

---

## Provenance

| Period | Who | What |
|---|---|---|
| 1989 | H. M. Pickett (JPL / Caltech, NASA contract NAS7-918) | Original Fortran programs |
| 22 Mar 1999 to 2008 | H. M. Pickett | Rewritten in C; the full changelog is in [`whats.new`](whats.new) |
| ~2008 | H. S. P. Müller (CDMS, Cologne) | Distributed as `Pickett_neu.zip` from the [CDMS Pickett page](https://cdms.astro.uni-koeln.de/classic/predictions/pickett/quelle/), the base of this repo |
| Oct 2020 | Kelvin Lee | Git import; SPCAT partition function now covers 1–1000 K in 1 K steps; test case added |

Git history:

```
f5af2ff Added test files and fixed bug with partition function calc
4066625 Added partition function range and gitignore   (also removed committed *.o)
e2a6f56 First commit based on HSPM's code from 2008
9352e8c first commit
```

---

## Repository layout

### Main programs (drivers)

| File | Program(s) | Role |
|---|---|---|
| [calfit.c](calfit.c) | `spfit`, `dpfit` | General non-linear least-squares line fitter (trust-region Marquardt–Levenberg) |
| [subfit.c](subfit.c) | (linked with calfit) | Helpers for the fitter: parameter labels (`getlbl`), block lookup, `.par`→`.bak` backup (`filbak`), correlation printout |
| [calcat.c](calcat.c) | `spcat`, `dpcat` | General frequency/intensity calculator; writes the `.cat` catalog and the partition function |
| [sortsub.c](sortsub.c) | (linked with calcat, sortn) | External merge/heap sort used to sort the `.cat` file at the end of SPCAT |
| [calmrg.c](calmrg.c) | `calmrg` | Merges a calculated `.cat` with experimental `.lin` lines |
| [calcatm.c](calcatm.c), [calfitm.c](calfitm.c) | `spcat`/`dpcat`, `spfit`/`dpfit` | Two-line `main()`s that call `calcat_run()` / `calfit_run()` |
| [pickett.h](pickett.h) | (library) | Public C entry points: `calcat_run`, `calfit_run`, `pk_library_mode`, `pk_fail` (see [Library entry points](#library-entry-points-picketth)) |
| [pkselftest.c](pkselftest.c) | `pkselftest_sp`, `pkselftest_dp` | Test driver that runs the library twice in one process (see [Regression tests](#regression-tests)) |

### Hamiltonian modules (one is linked into each fitter/calculator)

| File | Used by | Physics |
|---|---|---|
| [spinv.c](spinv.c) + [spinit.c](spinit.c), [spinit.h](spinit.h) | `spfit`, `spcat` | Asymmetric/symmetric rotors and linear molecules with up to ~999 vibrational states and multiple spins, l-doubling, n-fold symmetric tops, Euler/Fourier series, Itot spin basis. `spinit.c` handles the spin eigenfunctions for sets of equivalent spins. |
| [dpi.c](dpi.c) | `dpfit`, `dpcat` | Doublet-Π (²Π) states with one nuclear spin |

### Shared library (`splib.a`)

| File | Contents |
|---|---|
| [ulib.c](ulib.c) | Generic utilities: diagonaliser (`hdiag`, `triag`), eigenvector ordering (`ordblk`), `.par`/`.var` I/O (`getpar`, `getvar`, `putvar`), `.lin` reading (`getlin`), BCD parameter-ID handling, error propagation (`calerr`) |
| [cnjj.c](cnjj.c), [cnjj.h](cnjj.h) | 3j, 6j and 9j angular-momentum coupling coefficients (log-factorial tables) |
| [slibgcc.c](slibgcc.c), [slib.h](slib.h) | System-dependent code: command-line/interactive file-name handling (`filget`), `fopenq`, memory sizing, time stamps. Also the library run control: `pk_fail`, memory files, message capture, result callbacks |
| [pktmp.c](pktmp.c) | `pk_tmpfile()`, a `tmpfile()` that works for ordinary users on Windows; `pk_fpu_enter/leave()`, the fixed x87 precision for each run |
| [pktls.h](pktls.h) | `PK_TLS`: thread-local storage for the library state |
| [catutil.c](catutil.c), [catutil.h](catutil.h) | Quantum-number parsing (`readqn`), `pcard` free-format numeric parsing, catalog-format helpers |
| [lsqfit.c](lsqfit.c), [lsqfit.h](lsqfit.h) | Least-squares kernel (Givens rotations, Marquardt step, rank-deficient handling) |
| [dblas.c](dblas.c), [cblas.h](cblas.h) | Small bundled subset of CBLAS (`cblas_ddot`, `daxpy`, `drot`, …). It can be replaced by ATLAS or a system BLAS. |

### Documentation and data

| File | Contents |
|---|---|
| [calpgm.pdf](calpgm.pdf) | Suite overview, trust-region fitting notes, installation notes |
| [spinv.pdf](spinv.pdf) | **Main reference** for SPFIT/SPCAT: quantum-number formats, `.lin`, `.par/.var`, `.int`, `.cat`, `.str`, `.egy` formats, operator and parameter IDs, special cases |
| [dpi.pdf](dpi.pdf) | Reference for DPFIT/DPCAT parameters |
| [whats.new](whats.new) | Upstream changelog, 1999–2007 |
| [test/](test/) | CH₃CN prolate symmetric-top reference case (see [below](#test-case-and-verification)), plus the golden-output regression suite in `test/golden/` and its runner `test/run_golden.py` (see [Regression tests](#regression-tests)) |
| [README.md](README.md) | Short project description |

---

## Architecture

The suite follows a **driver + plug-in Hamiltonian** design. The generic drivers (`calfit.c`, `calcat.c`) know nothing about molecular physics. They call a fixed set of functions declared in [calpgm.h](calpgm.h) under "SPINV or DPI interface". At link time you pick `spinv.o + spinit.o` or `dpi.o` to supply those functions.

```mermaid
flowchart LR
    subgraph Drivers
        CF[calfit.c + subfit.c]
        CC[calcat.c + sortsub.c]
    end
    subgraph "Hamiltonian module (pick one)"
        SP[spinv.c + spinit.c]
        DP[dpi.c]
    end
    LIB[(splib.a<br/>ulib · cnjj · slibgcc<br/>catutil · lsqfit · dblas)]
    CF --> SP & DP
    CC --> SP & DP
    CF & CC & SP & DP --> LIB
    CF -. "+spinv" .-> SPFIT([spfit])
    CF -. "+dpi" .-> DPFIT([dpfit])
    CC -. "+spinv" .-> SPCAT([spcat])
    CC -. "+dpi" .-> DPCAT([dpcat])
```

### The Hamiltonian interface (`calpgm.h`)

| Function | Purpose |
|---|---|
| `setopt` | Read the option line(s) of the `.par`/`.var` file (spins, vibrational states, K range, symmetry, DIAG, …) |
| `setfmt` | Pick the quantum-number format code (`IQNFMT`/`QNFMT`) |
| `setblk` | Set up Hamiltonian blocks from the parameter list; also releases storage when called with `npar = 0` |
| `hamx` | Build and diagonalise one Hamiltonian block. Returns energies, eigenvectors, derivatives dE/dp and mixing coefficients. |
| `getqn` | Return the quantum numbers and degeneracy for a state (block, index) |
| `setint` | Read dipole definitions from the `.int` file |
| `intens` | Compute transition dipole matrix elements between two blocks |

Parameter and dipole identifiers are stored internally as packed **BCD** (`bcd_t`, see `getbcd`/`putbcd` in `ulib.c`). This lets IDs hold more digits than a `long` can, for example for `nvib > 99`.

### Library entry points (`pickett.h`)

The programs can be called as functions, so other code can run them in its own process. The command-line programs are two-line `main()`s in `calcatm.c` / `calfitm.c`.

| Function | Purpose |
|---|---|
| `int calcat_run(int argc, char *argv[])` | SPCAT, or DPCAT when linked with `dpi.c`. The arguments are the same as on the command line. Returns 0 or a failure status. |
| `int calfit_run(int argc, char *argv[])` | SPFIT, or DPFIT when linked with `dpi.c`. |
| `void pk_library_mode(int on)` | `on != 0`: don't install the Ctrl-C (SIGINT) handler, so the host program keeps its own. |
| `void pk_fail(int code)` | Used inside the library in place of `exit(EXIT_FAILURE)`. |

How a fatal error is handled:
1. Every `exit(EXIT_FAILURE)` in the library sources is now `pk_fail()`. This covers `calcat.c`, `calfit.c`, `subfit.c`, `spinv.c`, `spinit.c`, `dpi.c`, and `fopenq()`/`mallocq()` in `slibgcc.c`.
2. Each run function sets a `setjmp` point. `pk_fail()` jumps back to it, and the run function returns the status instead of ending the process. The error message is still printed on stdout.
3. Files opened with `fopenq()` are tracked: `calpgm.h` maps `fclose` to `pk_fclose`. When a run ends, successfully or not, the files that are still open get closed.

State that would leak from one run into the next is reset at the start of each SPCAT run:
- `qsum` (the partition-function sums)
- the `ibufof()` scratch-buffer state

SPFIT's `lbufof()` already resets itself. `python test/run_golden.py --inprocess` checks this (see [Regression tests](#regression-tests)).

#### Memory files, messages and result callbacks

A host can run the programs without files or console output. Everything is set up with these functions, also in `pickett.h`, and it works by intercepting I/O at three points, so the calculation code itself is unchanged.

| Function | Purpose |
|---|---|
| `pk_vfs_begin()` / `pk_vfs_end()` | Turn memory files on/off (`end` frees them) |
| `pk_vfs_put(name, data, len)` | Provide an input, for example `"mol.int"` |
| `pk_vfs_get(name, &data, &len)`, `pk_vfs_count()`, `pk_vfs_name(k)` | Read the outputs after the run |
| `pk_log_mode(PK_LOG_STDOUT / PK_LOG_CAPTURE / PK_LOG_DISCARD)`, `pk_log_text(&len)` | Where the program's messages go |
| `pk_set_sink(&sink)` | Register `PK_SINK` callbacks for full-precision results |

How each piece works:

1. **Memory files.** `fopenq()` and the catalog sort (`sortsub.c`) open files through `pk_fopen()`.
   - While memory files are on, a read gives a temporary stream filled with the named buffer.
   - A write goes to a temporary stream whose contents become the buffer when it is closed.
   - So `filbak()` (`.par` → `.bak`), the `.bak` read and the in-place `.cat` sort all work unchanged.
   - Parameter-label files (`*.nam`) that were not put are still read from disk.
2. **Messages.** `calpgm.h` maps `stdout`, `printf` and `puts` to `pk_stdout()` / `pk_puts()`. By default they still print on stdout.
3. **Callbacks.** Each is called where the program writes the corresponding line:

| Callback | Called for |
|---|---|
| `cat_line` | Every catalog line: frequency, error, log intensity, S·μ², E_low, g_up, tag, QNFMT, both states' quanta. Calculation order; the `.cat` file is sorted afterwards |
| `level` | Every energy level, whether or not `.egy` is written |
| `partition` | Each row of the Q(T) table |
| `fit_iteration` | The statistics at the end of each SPFIT iteration |
| `fit_param` | Each parameter as written to `.par`/`.var`, with the fitted and the `.par` error |

The values are full `double`s, not the rounded file fields. For example, `.cat` keeps only 4 decimals of the frequency.

**`pk_tmpfile()` ([pktmp.c](pktmp.c)).** On Windows, the C runtime's `tmpfile()` creates its file in the root of the current drive (`C:\`), which ordinary users usually cannot write to, so it returns NULL. This affected the original programs too: the `.cat` sort of large catalogs, the `ibufof()` disk buffer and SPFIT's line buffer all use `tmpfile()`. `calpgm.h` now maps `tmpfile()` to `pk_tmpfile()`, which uses the user's temporary folder (`GetTempPath`) and deletes the file when it is closed. On other systems it is `tmpfile()`.

#### Threads (`pktls.h`)

Different threads can run programs at the same time. They get the same results as a single thread, bit for bit.

**How it works.** The programs keep their working state in static variables, 135 of them in the library sources. Instead of moving them into a context struct, which would change the signature of almost every function, every mutable static is declared `static PK_TLS`. That is `_Thread_local` (C11), or `__thread` / `__declspec(thread)`, so each thread gets its own copy.
- 91 statics are thread-local.
- Only read-only constants and tables stay shared: `zero`, `pten`, the symmetry tables in `spinv.c`, and so on. Each was checked for direct writes and for writes through its address. `spinv.c`'s `zero` and `szero` are passed as possible outputs to `specfc()`/`dircos()`, so they are thread-local too.
- `pk_thread_safe()` returns 1 when the build has thread-local state. Define `PK_NO_TLS` to build without it; it then returns 0, and the Rust layer serializes runs with a lock.

Changes the conversion needed or uncovered:

| Change | Why |
|---|---|
| `vinfo` / `dipinfo` start as NULL instead of `&vinfo1` / `&dipinfo0` | The address of a thread-local object is not a constant initializer. `setopt()` / `setint()` always set them before use |
| `csym` in `setblk()` is a local array | It was a static that was swapped for every oblate rotor, so a second oblate run in the same process printed swapped symmetry labels |
| `ixphase` is reset in `setopt()` | `setint()` adds to it for non-standard phase conventions, so it built up from run to run |
| `pk_fpu_enter()` / `pk_fpu_leave()` ([pktmp.c](pktmp.c)) around every run | The math library uses x87 instructions on MinGW. The C runtime sets extended precision for the main thread, but new Windows threads start at 53-bit precision, which changed the last digits of results computed on other threads (e.g. S·μ² 4.876953976515408 vs …411). The run sets the runtime's default control word `0x037F` and restores the caller's |
| `chtime()` uses `localtime_r` / `asctime_r` outside Windows | glibc's `localtime` / `asctime` share one buffer. The Microsoft runtime already uses per-thread buffers |

**Speed.** The 64-bit gcc 16 build uses native Windows TLS (`.tls$`), so a single run is as fast as before: SPCAT prolate 604 ms against 595–675 ms before the change, asym 32 ms against 31–37 ms. gcc 6.3 (32-bit) emulates TLS with `__emutls` calls; this was not timed.

**Check that the tests catch races.** If `qsum`/`temp` in `calcat.c` are made shared again, `cargo test -p pickett-sys --test threads` reports wrong results on several threads.

Limits of this layer, to be removed in later phases:
- **Memory leaks on failure.** If a run fails in the middle of a calculation, the memory it allocated is not freed.
- **Results are partly text.** The calculation still formats its output files internally; memory files only redirect them. The callbacks cover catalog lines, levels, Q(T) and fitted parameters. Per-line fit residuals (obs−calc, including blends) are only in the `.fit` text so far.

### Data flow

```mermaid
flowchart TB
    LIN[".lin<br/>assigned lines"] --> SPFIT
    PAR[".par<br/>parameters + options"] --> SPFIT
    SPFIT --> FIT[".fit<br/>fit report"]
    SPFIT --> NPAR[".par (updated)<br/>+ .bak (previous)"]
    SPFIT --> VAR[".var<br/>params + covariance"]
    VAR --> SPCAT
    INT[".int<br/>dipoles, T, limits"] --> SPCAT
    SPCAT --> CAT[".cat<br/>predicted catalog"]
    SPCAT --> OUT[".out<br/>log + partition fn"]
    SPCAT --> EGY[".egy<br/>energy levels (opt.)"]
    SPCAT --> STR[".str<br/>line strengths (opt.)"]
    CAT --> CALMRG
    LIN --> CALMRG
    CALMRG --> MRG[".mrg / .new / .log"]
    STR --> STARK --> STK[".stk"]
```

---

## Building

### With `make` (Linux/macOS/MSYS2, or `mingw32-make` on Windows)

The active makefile is [Makefile](Makefile) (gcc, `-g -O3 -Wall`):

```sh
make            # spfit spcat calmrg dpfit dpcat calbak
make all        # also iamcalc moiam stark termval sortn reassign sortegy iambak
make selftest   # pkselftest_sp, pkselftest_dp (in-process test drivers)
make install    # moves the default executables to /usr/local/bin
make clean      # removes executables, *.o and splib.a
```

`make all` was tested with `mingw32-make` and MinGW gcc 6.3 on Windows 11, and all 14 programs build. Add `CFLAGS="-O2 -w"` on the command line to hide the legacy-C warnings.

BLAS selection (edit `BLASLIB` at the top of the Makefile):

| Setting | Effect |
|---|---|
| `BLASLIB=` (default) | Uses the bundled `dblas.c` |
| `BLASLIB=-lcblas -latlas` | ATLAS |
| `BLASLIB=-lcblas -lblas` | Reference Fortran BLAS + CBLAS wrappers |

The bundled `dblas.o` goes into `splib.a` only when `BLASLIB` is empty.

[Makefile.txt](Makefile.txt) is an older alternate makefile that the main `Makefile` now covers. [makefile.linux](makefile.linux) is an obsolete g77/Fortran-era makefile; see [Legacy files](#legacy-alternate-and-unused-files).

### Without `make` (e.g. Windows + MinGW gcc)

The core programs are self-contained, so plain gcc works. These commands were tested with MinGW gcc 6.3 on Windows 11:

```sh
LIB="ulib.c cnjj.c slibgcc.c catutil.c lsqfit.c dblas.c"
gcc -O2 -o spfit calfitm.c calfit.c subfit.c spinv.c spinit.c $LIB -lm
gcc -O2 -o spcat calcatm.c calcat.c sortsub.c spinv.c spinit.c $LIB -lm
gcc -O2 -o dpfit calfitm.c calfit.c subfit.c dpi.c $LIB -lm
gcc -O2 -o dpcat calcatm.c calcat.c sortsub.c dpi.c $LIB -lm
gcc -O2 -o calmrg calmrg.c $LIB -lm
```

The code is K&R/ANSI C from ~1999–2008, so a modern compiler prints many warnings (implicit `int`, old-style definitions). Add `-w` to silence them. gcc 15 and later default to C23, which rejects the old-style declarations: add `-std=gnu17`. That was tested with MSYS2's 64-bit gcc 16.2, and it is what `pickett-sys` uses.

### Memory limits

All matrices are allocated dynamically. The heap-size hints `NDHEAPF`/`NDHEAPC` in `calpgm.h` (1.2 M) and `maxmem()` in `slibgcc.c` control the working-store size.

---

## Running the programs

All programs share the file-name handling in `filget()` ([slibgcc.c](slibgcc.c)):

* **One base name** gives every file. `spcat prolate` reads `prolate.int` and `prolate.var` and writes `prolate.out`, `prolate.cat`, and so on.
* **Explicit names** override single files: any argument whose extension matches a known one replaces that file. For example, `spfit mymol other.lin` uses `other.lin` but `mymol.par`.
* **Options**: an argument starting with `-` is passed to the program as an option string.
* **No arguments**: the program prompts `Enter file name` interactively.
* **`SPECNAME`** environment variable: the extra directory searched for `*.nam` parameter-label files when they are not in the current directory. The trailing path separator is required, e.g. `export SPECNAME=/home/me/spectra/`.

| Program | Inputs | Outputs |
|---|---|---|
| `spfit` / `dpfit` | `.par`, `.lin` | `.fit`, `.par` (new), `.bak` (old `.par`), `.var` |
| `spcat` / `dpcat` | `.int`, `.var` | `.out`, `.cat` (sorted), optional `.egy`, `.str` |
| `calmrg` | `.cat`, `.lin` (+ `.opt`) | `.mrg`, `.new`, `.log` |
| `termval` | `.lin` | `.trm`, `.lgy` |
| `stark` | `.str` | `.stk` |
| `sortegy` | `.egy`, `.inp`, `.opt` | `.lgy`, `.lin`, `.log` |
| `iamcalc` | `.iam`, `.par` | `.out`, `.int`, `.bak` |
| `moiam` | `.inp` | `.iam`, `.moi`, `.xyz` |
| `sortn` | `infile [outfile]` | sorted file |
| `reassign` | `keyfile infile outfile` | reassigned line file |
| `cnvir` | `input output` | catalog file with frequencies in cm⁻¹ |

### Typical workflow

1. Write `mol.par` (title, control line, option line(s), parameters) and `mol.lin` (assigned transitions).
2. Run `spfit mol`. Inspect `mol.fit`. Iterate. SPFIT backs up the old `.par` to `.bak` each run.
3. Copy or rename `mol.var` (or `mol.par`) for SPCAT and write `mol.int` (dipoles, temperature, limits).
4. Run `spcat mol`. This produces `mol.cat`, plus the partition-function table at the end of `mol.out`.
5. Optionally run `calmrg` to merge experimental lines into the catalog.

---

## File formats (summary)

Free-format fields are parsed by `pcard()`. The definitive descriptions are in [spinv.pdf](spinv.pdf). The examples below come from [test/](test/).

### `.par` / `.var` (parameters)

```
CH3CN                                                   Mon Jul 20 17:14:58 2009   <- line 1: title
  30  254    5    0    0.0000E+000    1.0000E+003    1.0000E+000 1.0000000000      <- line 2
s  -1    1   0  100   0   1   1   1     0    1   0                                  <- line 3: options
        10099  1.489000744965263E+005 1.00000000E-035 /A-B                          <- parameters
```

* **Line 2:** `NPAR NLINE NITR NXPAR THRESH ERRTST FRAC CAL`. These are the maximum number of parameters, maximum number of lines (negative = up to 10 quanta/state), number of iterations (negative = line-assignment diagnostics), parameters to exclude for special lines, initial Marquardt–Levenberg parameter, maximum (obs−calc)/error, error scaling (negative = "convention" scaling), and IR frequency scaling.
* **Option line(s):** `CHR SPIND NVIB KNMIN KNMAX IXX IAX WTPL WTMN VSYM EWT DIAG XOPT`. `CHR` is the option-line character. A leading `s`/`a`/`l` or similar selects symmetric-top quanta. Repeating the option line defines more vibronic states.
* **Parameter lines:** `IDPAR VALUE ERROR /LABEL`. A negative `IDPAR` ties a parameter to the previous one with the ratio given as the value. In a `.var` file, the parameter block is followed by the packed correlation/covariance matrix that SPCAT uses for uncertainties.

### `.lin` (assigned lines, for SPFIT)

Fixed-format quantum numbers (`12I3`: upper quanta, then lower) followed by free-format `FREQ ERR WT`. A blend shares the same frequency. A negative error means wavenumbers (see `getlin` in `ulib.c`).

### `.int` (intensity control, for SPCAT)

```
CH3CN                                                                  <- title
1  41505    7820.3911       0    135   -11.0    -11.0    1000. 300.    <- control line
  001  3.92197                                                         <- one line per dipole: IDIP  value (Debye)
```

Control-line fields (the `dvec[]` order in `calcat.c`, defaults in brackets):

| # | Field | Meaning |
|---|---|---|
| 0 | `FLAGS` [0] | Output flags, decoded digit by digit: ≥1000 IR (cm⁻¹) units. Hundreds digit = 1 prints energies to `.out`. Tens digit ≥ 1 writes `.str`; 2 writes the dipole components separately. Units digit ≠ 0 writes `.egy`; 2/4 adds derivatives, ≥3 adds eigenvectors, ≥5 dumps the Hamiltonian. |
| 1 | `TAG` [999] | Species tag written to `.cat` |
| 2 | `QROT` [1000] | Partition function used for intensities (e.g. at 300 K) |
| 3 | `FBGN` [0] | Minimum quantum number (block) |
| 4 | `FEND` [0] | Maximum quantum number (block) |
| 5 | `STR0` [−100] | Minimum log₁₀ intensity |
| 6 | `STR1` [−100] | Minimum log₁₀ intensity × (300 GHz / ν)² |
| 7 | `FQLIM` [9999.99] | Maximum frequency (GHz, or cm⁻¹ for IR) |
| 8 | `TEMP` [300] | Temperature (K) for intensities |
| 9 | `MAXV` [−1] | Maximum vibrational state |

### `.cat` (catalog, output of SPCAT)

Standard JPL/CDMS catalog format, fixed width:

```
   18397.7830  0.0000 -5.3539 3    0.0000  3  41505 202 1 0         0 0
FREQ(F13.4) ERR(F8.4) LGINT(F8.4) DR(I2) ELO(F10.4) GUP(I3) TAG(I7) QNFMT(I4) QN'(6I2) QN"(6I2)
```

`LGINT` is log₁₀ of the integrated intensity in nm² MHz at `TEMP`. `DR` is the degrees of freedom of the rotational partition function. `ELO` is the lower-state energy in cm⁻¹. `QNFMT = Q·100 + H·10 + NQN` gives the quantum-number format. The file is sorted by frequency at the end of the run (`sortn()`).

### `.egy` (energy levels, optional)

```
     6    1          5.273508          0.000000   1.000000    3:  1 -1
BLOCK INDEX       ENERGY(cm-1)       ERROR       MIXING   DEGEN: quanta
```

### `.out` (SPCAT log)

Echoes the inputs, parameters, block structure and line count. It ends with the **partition-function table** (`TEMPERATURE - Q(SPIN-ROT.) - log Q(SPIN-ROT.)`). Q is relative to the lowest energy level found.

### `.fit` (SPFIT report)

Per-iteration obs−calc table, blend handling, parameter values and uncertainties, correlation matrix, and RMS / microwave-RMS statistics.

---

## Fork modification: extended partition function

**Commits:** `4066625`, `f5af2ff` (Kelvin Lee, Oct 2020). **File:** [calcat.c](calcat.c), near lines 55–170.

Upstream SPCAT evaluates the spin-rotation partition function at 8 fixed temperatures: 300, 225, 150, 75, 37.5, 18.75 and 9.375 K, plus the `.int` `TEMP`. This fork replaces them with a dense grid, so Q(T) can be tabulated without re-running SPCAT at each temperature:

```c
#define NTEMP 1001
#define TMAX  1000
static double temp[NTEMP], qsum[NTEMP];
...
for (i = 0; i < NTEMP; i++)
    temp[i] = (double)(TMAX - i);       /* 1000, 999, ..., 1, 0 K */
```

After the grid is built, the upstream merge logic still runs:

* If `TEMP` from the `.int` file matches a grid point (within 0.01 K), the table uses the first 1000 entries: **1000 K → 1 K**, 1 K steps. The 0 K slot is dropped.
* If `TEMP` is off-grid (e.g. 2.725 K or 1500 K), it overwrites the 0 K slot and is sorted into place. The table then has 1001 entries, and its lowest temperature is 1 K or `TEMP`, whichever is lower.

The table goes to stdout and to the end of the `.out` file, one row per temperature:

```
TEMPERATURE - Q(SPIN-ROT.) - log Q(SPIN-ROT.)
   1000.000     47744.6549    4.6789
    999.000     47672.9235    4.6783
   ...
      1.000         2.6347    0.4207
```

To change the range, edit `TMAX` and set `NTEMP = TMAX + 1`. The grid step is fixed at 1 K. Each extra temperature adds one `exp()` per energy level, and for large catalogs this is the most expensive part of SPCAT (see [Speed of SPCAT](#speed-of-spcat)). **Note:** Q is only meaningful if the calculation includes all levels that are populated at the highest temperature. Make sure `FEND`, `KNMAX` and the vibrational states you include are large enough for high-T values.

The `QROT` value on the `.int` line still sets the `.cat` intensities. The tabulated Q values are diagnostic output only and are **not** fed back automatically.

### Speed of SPCAT

**Where the time goes.** I profiled a copy of the code with timing wrappers around `hamx`, `hdiag`, `intens` and `calerr`, and timers around the loops in `calcat.c`. This is SPCAT on the golden cases, 64-bit gcc 16, `-O2`:

| Part | CH₃CN prolate (17,236 levels) | SO₂-like asym (221 levels) |
|---|---|---|
| Partition-function sums, 1000 temperatures | 0.41 s (10.2 M `exp`) — about 60% | 0.015 s |
| Printing the 1000-row Q table to stdout (a pipe) | 0.09 s | 0.08 s |
| Hamiltonian set-up and diagonalisation (`hamx`, incl. `hdiag`) | 0.009 s — **1.4%** | < 0.001 s |
| Intensities (`intens`) and line errors (`calerr`) | 0.005 s | < 0.001 s |
| Total | 0.69 s | 0.13 s |

The largest matrix in these cases is 51×51. For SPFIT, `hamx` is 4–15% of a 13–16 ms run. Replacing the 1968 EISPACK diagonaliser (`hdiag`/`triag`) with LAPACK would therefore gain almost nothing here. It would also mean a new dependency and a risky change to the eigenvector order that `ordblk()` uses to assign quantum numbers. That change only becomes worth it for models with much larger blocks; profile such a model first.

**What was done instead: parallel partition sums, with unchanged results.**
- The level loop no longer updates all 1000 sums for every level. It records one event per level, and one per shift of the energy zero (`pf_add`).
- At the end, `pf_compute()` runs the original loop on those events, levels outside and decreasing temperatures inside, with the early `break` kept.
- The temperatures are split into interleaved subsets (`k = part, part + n, …`), one per thread (`pk_run_parallel` in [pktmp.c](pktmp.c)).
- Each temperature still gets the same operations in the same order, so the sums are **bit-identical** to the old code. This was checked by comparing the `.out`, `.cat`, `.egy` and `.str` of all three SPCAT/DPCAT golden cases, built from the previous commit and from this one with the same compiler, with 1 and 8 threads (22 comparisons, all identical).
- Workers set the same x87 precision as the run (`pk_fpu_enter`).
- Small jobs (fewer than 2×10⁶ level×temperature pairs) stay on one thread, because starting threads cost more for them.

**Choosing the number of threads.** In order of precedence:
- `pk_set_threads(n)` from C, or `pickett_sys::set_threads(n)` from Rust;
- the `PICKETT_THREADS` environment variable;
- otherwise one thread per core.

The setting belongs to the calling thread. `pickett::predict_parallel` sets 1 in its workers, because there every core already runs a whole prediction.

**Measured** (8 cores, Windows 11; the machine had 25–40% background load, so single numbers vary by up to ±30%):

| | prolate (2314 lines) | asym (198 lines) |
|---|---|---|
| Executable, previous version vs new, mean of 5 interleaved runs | 1461 ms → 1452 ms (1 thread) / 934 ms (all cores) | — |
| `pickett::predict`, 1 thread → all cores | 0.94–1.11 s → 0.56–0.80 s (1.2–2.0×) | unchanged (stays on one thread) |
| `predict_parallel`, 8 at once, per prediction | 0.36–0.40 s | 28 ms |

```sh
cargo run --release -p pickett --example bench -- ../test/golden/spcat_prolate/input/prolate.var ../test/golden/spcat_prolate/input/prolate.int 16 8
```

---

## Test case and verification

[test/](test/) holds a CH₃CN ground-state prolate symmetric-top prediction:

| File | Role |
|---|---|
| `prolate.var` | Rotational constants (A−B, B, D_K, D_JK, D_J), symmetric-top option line |
| `prolate.int` | μ = 3.92197 D, TAG 41505, J ≤ 135, ν ≤ 1000 GHz, T = 300 K |
| `prolate.out` | Reference log, including the 1000 → 1 K Q table |
| `prolate.cat` | Reference catalog (2314 lines) |
| `prolate.egy` | Reference energy levels |

To reproduce the reference outputs:

```sh
mkdir run && cp test/prolate.int test/prolate.var run/ && cd run
../spcat prolate
diff prolate.cat ../test/prolate.cat     # expect no differences
diff prolate.egy ../test/prolate.egy     # expect no differences
diff prolate.out ../test/prolate.out     # only the timestamp line differs on Linux
```

This was checked on Windows 11 with MinGW gcc 6.3: `.cat` and `.egy` are byte-identical to the reference. `.out` differs only in the run timestamp and in Windows' three-digit exponent format (`E+005` vs `E+05`) in the echoed parameter table.

### Regression tests

[test/run_golden.py](test/run_golden.py) checks the programs against known-good outputs in [test/golden/](test/golden/). It needs Python 3.6+ (standard library only), `make` (or `mingw32-make`) and gcc. One command builds and runs every case and reports pass/fail per file:

```sh
python test/run_golden.py
```

The cases cover SPCAT and SPFIT (the prolate case above, plus an asymmetric top), DPCAT and DPFIT (a ²Π state with hyperfine), and calbak. [test/golden/README.md](test/golden/README.md) describes each case and how its inputs were made. Each case runs in a fresh temporary directory, which is kept for inspection if anything fails. The exit status is non-zero on any failure.

How files are compared:

* `.cat` and `.egy` must match exactly. Only CRLF vs LF is ignored.
* All other files are compared token by token. Timestamps are masked and Windows' 3-digit exponents (`E+005`) are treated as `E+05`. Integers must be equal. Floats must agree within `max(rtol·|x|, 1 unit in the last printed digit)`, with `rtol = 1e-6` by default.
* `.str` dipoles are compared by magnitude, because their sign follows the arbitrary eigenvector phase.

Useful options:

```sh
python test/run_golden.py -k spfit                      # only cases whose name contains "spfit"
python test/run_golden.py --no-build                    # use the executables already built
python test/run_golden.py --clean --make-arg CFLAGS=-O0 # clean rebuild with other flags
python test/run_golden.py --update -k dpfit             # accept new outputs (review the git diff!)
```

All cases pass with the default `-O3` (x87) build, and also with `-O0` and `-O2 -msse2 -mfpmath=sse` builds (MinGW gcc 6.3, Windows 11). The SSE build changes the fitted constants by about 1e-10 relative, which is well inside the tolerance. Any change to the C code should keep `python test/run_golden.py` passing.

**In-process mode.** `--inprocess` checks the library entry points instead of the executables:

```sh
python test/run_golden.py --inprocess
```

SPCAT, SPFIT, DPCAT and DPFIT steps then run through `pkselftest_sp` / `pkselftest_dp` (built with `make selftest`). In one process, the driver:
1. calls the run function on a missing file and checks that it returns a failure status, not an exit;
2. runs the case;
3. restores the input files;
4. runs the case again.

The outputs compared are those of the second run, so state left over from the first run, or from the failure, shows up as a difference. The check is known to work: with the `qsum` reset in `calcat.c` removed, the second SPCAT run reports the partition function doubled and `prolate.out` fails.

---

## Auxiliary programs

| Program | Source | Purpose |
|---|---|---|
| `calmrg` | [calmrg.c](calmrg.c) | Merges `.cat` predictions with experimental `.lin` data. Replaces calculated frequencies with measured ones and writes `.mrg`, plus `.new`/`.log` for unmatched or bad lines. |
| `calbak` | [calbak.c](calbak.c) | Converts catalog-format lines back into a `.lin` file for SPFIT |
| `termval` | [termval.c](termval.c) | Derives term values (energy levels) from a network of assigned lines (`.lin` → `.trm`, `.lgy`) |
| `stark` | [stark.c](stark.c) | Computes Stark coefficients from the `.str` line-strength file → `.stk` |
| `sortn` | [sortn.c](sortn.c) + [sortsub.c](sortsub.c) | Sorts catalog-format files by frequency |
| `sortegy` | [sortegy.c](sortegy.c) | Sorts and relabels `.egy` energy files using rules in `.inp`/`.opt` |
| `reassign` | [reassign.c](reassign.c) | Rewrites quantum-number assignments in a line file according to a key file |
| `iamcalc` | [iamcalc.c](iamcalc.c) + [ftran.c](ftran.c), [readopt.c](readopt.c) | Internal-axis-method (IAM) internal-rotation calculations; generates `.par`/`.int` input for SPFIT/SPCAT |
| `iambak` | [iambak.c](iambak.c) | Converts IAM fit parameters back to `.iam` input |
| `moiam` | [moiam.c](moiam.c) + [ftran.c](ftran.c) | Internal-rotation structural parameters (moments of inertia) from geometry |
| `cnvir` | [cnvir.c](cnvir.c) | Converts catalog-format frequency and error fields from MHz to cm⁻¹ |
| `hundscvt` | [hundscvt.c](hundscvt.c) | Converts quantum-number labels between Hund's case (a) and (b) |

The JPL catalog-access utilities ([catread.c](catread.c), [catread.h](catread.h), [catlist.c](catlist.c), [catform.c](catform.c)) read the JPL catalog directory structure. `catform.c` is a CGI web-form handler. They use old K&R C and `gets()`, and none of the makefiles builds them.

---

## Rust file library: pickett-io

[rust/pickett-io](rust/pickett-io) is a Rust crate that reads and writes the suite's files without running the programs. It has no dependencies. It is step 1 of turning the suite into a library; later steps will call the C Hamiltonian code in-process.

| Type | Files | Reader follows | Writer format |
|---|---|---|---|
| `ParFile` | `.par`, `.var`, `.bak` | `calfit.c` line 2, `setopt()` option lines, `getpar()`, `getvar()` | SPFIT (`calfit.c`, `putvar()`) |
| `LinFile` | `.lin` | `getlin()` | `calbak.c` |
| `IntFile` | `.int` | `calcat.c` | plain, lossless (no C program writes `.int`) |
| `CatFile` | `.cat` (also `.mrg`) | fixed JPL/CDMS columns, `readqn()` | SPCAT (`calcat.c`, `gupfmt()`) |
| `EgyFile` | `.egy` | `calcat.c` level lines; other lines kept as text | SPCAT |

The building blocks are public too:
- `pcard::pcard` is the C free-format number reader. Numbers can run together (`0.31-0.07`), `,,` skips a field and `/` ends the line.
- `ParamId` holds BCD parameter and dipole IDs as digit strings, so long IDs are not truncated.
- `qn` encodes the two-character quantum fields (`A0` = 100, `a1` = −11).
- `cfmt` gives C `printf` formats with a selectable `%E` exponent width.

```sh
cd rust
cargo test                                   # unit tests + golden round trips
cargo run --example cat_summary -- ../test/golden/spcat_prolate/expected/prolate.cat
cargo run --example roundtrip -- ../test/golden/spfit_asym/expected/asym.var
cargo doc --open                             # API documentation
```

```rust
use pickett_io::{CatFile, Flavor, ParFile};

let var = ParFile::parse(&std::fs::read_to_string("mol.var")?, Flavor::Spinv)?;
for p in &var.params {
    println!("{} {} ± {:?} {}", p.id, p.value, p.error, p.label);
}
let cov = var.covariance();          // packed upper triangle, as SPFIT writes it
let cat = CatFile::parse(&std::fs::read_to_string("mol.cat")?)?;
```

Points to know:
- **Option lines depend on the module.** Pass `Flavor::Spinv` for SPFIT/SPCAT files: option lines continue while `VSYM` is negative. Pass `Flavor::Dpi` for DPFIT/DPCAT files: there is exactly one option line.
- **Round-trip guarantee.** `tests/golden.rs` reads all 29 `.par/.var/.bak/.lin/.int/.cat/.mrg/.egy` files under `test/golden/`, writes them back and parses them again. The data must be unchanged in every case. All 25 files written by the C programs come back byte-identical. The exceptions are the 3 `.int` files and one hand-typed `.var` value, as listed in `reformatted_ok()`.
- **Exponent width.** The old Microsoft C runtime used by MinGW writes 3 exponent digits (`E+005`), while glibc writes 2. Each file type remembers what it read in `exp_digits` and writes it back the same way.
- **Number values** are the correctly rounded `f64` of the decimal text. The C reader builds numbers digit by digit, so in rare cases it can differ by one unit in the last place.
- **Not covered yet:** `.str`, `.fit`, `.out`, and files with more than 6 quanta per state in `.cat`. `LinFile::parse_with_nqn` does handle wider `.lin` files.

---

## Rust in-process runner: pickett-sys

[rust/pickett-sys](rust/pickett-sys) compiles the C code and links it into Rust, so SPCAT, SPFIT, DPCAT and DPFIT run inside the calling process (see [Library entry points](#library-entry-points-picketth)).

```rust
use pickett_sys::{run_in, Program};

// reads data/mol.int and data/mol.var, writes data/mol.cat, .out, ...
run_in(Program::Spcat, std::path::Path::new("data"), "mol")?;
// then read the results with pickett-io
let cat = pickett_io::CatFile::parse(&std::fs::read_to_string("data/mol.cat")?)?;
```

| Item | Notes |
|---|---|
| `run(program, args)` | Same arguments as the command line, e.g. `["mol"]` |
| `run_in(program, dir, base)` | Passes every `<dir>/<base>.<ext>` explicitly, so directory names with dots are safe |
| `set_threads(n)`, `threads()` | Threads one run on the calling thread may use for its own work (0 = one per core) |
| `run_memory(program, base, inputs)` | No files and no console output. Returns `MemoryRun`: `status`, `files` (every memory file after the run), `log` (the captured messages) and `records` (full-precision `cat_lines`, `levels`, `partition`, `fit_iterations`, `fit_params`) |
| `RunError::Failed { program, status }` | A fatal error in the C code. The process keeps running. The message is on stdout, or in `MemoryRun::log` |
| `ffi` | The raw `extern "C"` functions, including `PkSink` |

**How it builds.** `build.rs` has no dependencies. It calls gcc and `ar` directly and puts both variants into one `libpickett.a`:
- The SPCAT/SPFIT objects get run functions `pk_spcat_run` / `pk_spfit_run`.
- The DPCAT/DPFIT objects get `pk_dpcat_run` / `pk_dpfit_run`, and every global symbol of `calcat.c`, `calfit.c`, `subfit.c` and `dpi.c` is prefixed `dp_` with `-D` (the list is `DP_RENAMES`, found with `nm`).
- Everything is compiled with `-std=gnu17`.

**Compiler.** The build uses the first of these whose `gcc -dumpmachine` matches the target architecture: `PICKETT_CC`, `CC`, `C:\msys64\mingw64\bin\gcc.exe` (Windows), `gcc`. On Windows the Rust target must be `x86_64-pc-windows-gnu`, and the 32-bit MinGW gcc cannot be used for it. Install a 64-bit one with MSYS2:

```sh
pacman -S mingw-w64-x86_64-gcc
```

This was tested with gcc 16.2 (MSYS2) and Rust 1.100 on Windows 11. Linking works with Rust's own MinGW linker.

**Concurrency.** Each thread uses its own C state (see [Threads](#threads-pktlsh)), so calls from different threads run in parallel. `thread_safe()` tells whether the library was built that way. Build with `PICKETT_NO_TLS=1` to turn it off; calls are then serialized with a global mutex.

**Tests** (`cargo test -p pickett-sys`):
- Every SPCAT/SPFIT/DPCAT/DPFIT golden case runs twice in-process. `.cat`/`.egy` must match exactly, and `.par`/`.var`/`.bak` must match within `1e-6` relative. The 64-bit build fits A = 60778.54915641392 MHz against the golden 60778.54915641401.
- A failing run returns `Err`, and the next run still works.
- Three threads run at once.
- `tests/threads.rs`:
  - 8 threads each run all six SPCAT/SPFIT/DPCAT/DPFIT golden cases twice, mixing the programs, and every record and output file must equal the single-thread result exactly;
  - a thread that keeps failing does not disturb four others;
  - `thread_safe()` matches the build setting.
- A directory name with dots works.
- Deliberately corrupted expected files make the test fail, so the comparison is known to work.
- `tests/memory.rs` covers `run_memory`:
  - every golden case in memory gives the expected `.cat`/`.egy`/`.par`/`.var`;
  - no file appears in the working directory;
  - every `cat_line` record matches a `.cat` line once rounded;
  - `level` and `partition` records match `.egy` and the `.out` table, all 1000 rows;
  - `fit_param` records match `.var` to 2e-15, and `fit_iteration` matches the iterations printed in `.fit`;
  - a failure's message ends up in `log`.

---

## Rust high-level API: pickett

[rust/pickett](rust/pickett) is the API meant for applications. It takes and returns `pickett-io` types, and runs everything in memory through `pickett-sys`.

```rust
use pickett::{fit, predict, Flavor, IntFile, LinFile, ParFile};

let result = fit(Flavor::Spinv, &par, &lin)?;        // SPFIT
println!("RMS {:?}", result.rms_error());
let spectrum = predict(Flavor::Spinv, &result.var, &int)?;  // SPCAT with the fitted .var
let q300 = spectrum.partition_at(300.0);
```

| Item | Contents |
|---|---|
| `predict(flavor, &var, &int)`, `predict_text(flavor, var, int)` | `Prediction`: `lines` (full precision, sorted by frequency), `levels`, `partition`, `catalog` (the `.cat` as written), `report` (`.out`), `log`, `files`. `partition_at(T)` interpolates ln Q in ln T between table points |
| `fit(flavor, &par, &lin)`, `fit_text(flavor, par, lin)` | `FitResult`: `params` (full precision), `iterations`, `par` and `var` (as `ParFile`, ready for the next fit or `predict`), `report` (`.fit`), `log`, `files`. `rms_error()` |
| `predict_parallel(flavor, &jobs, threads)` | Many predictions at once, on all cores by default, each prediction on one thread. Results come back in job order and equal `predict` for each job. Use it for parameter scans and Monte-Carlo error estimates |
| `Error` | `Run` (with the program's `log`), `NoOutput` (e.g. SPFIT did no iteration), `Parse`, `BadArgument` |

`Flavor::Spinv` runs SPCAT/SPFIT and `Flavor::Dpi` runs DPCAT/DPFIT.

**Tests** (`cargo test -p pickett`):
- `predict_text` gives exactly the golden `.cat` for all three SPCAT/DPCAT cases.
- `fit` reproduces the golden parameters within 1e-6 relative for the three SPFIT/DPFIT cases.
- The `partition_at` interpolation and its out-of-range behaviour.
- Typed and text input give the same result.
- An error carries the program's message, and the next call works.
- Fit followed by predict: starting from perturbed constants and 50 lines with ±0.02 MHz noise, the fitted constants reproduce the reference SO₂-like catalog line for line, within 0.5 MHz.

```sh
cargo run -p pickett --example fit_predict -- ../test/golden/spfit_asym/input/asym.par \
    ../test/golden/spfit_asym/input/asym.lin ../test/golden/spcat_asym/input/asym.int
cargo run --release -p pickett --example bench -- ../test/golden/spcat_prolate/input/prolate.var \
    ../test/golden/spcat_prolate/input/prolate.int 16 8
```

`bench` times runs one after another and then spread over threads. On an 8-core Windows 11 machine (release build):

| Case | One run | 8 threads | Speed-up |
|---|---|---|---|
| CH₃CN prolate, 2314 lines | 604 ms | 158 ms per run | 3.8× |
| SO₂-like asymmetric top, 198 lines | 34 ms | 21 ms per run | 1.7× |

The small case gains less because much of its time goes into the temporary files behind the memory files.

---

## Rust spectrum analysis: pickett-spectrum

[rust/pickett-spectrum](rust/pickett-spectrum) turns a measured spectrum into assigned lines for SPFIT. It is pure Rust with no dependencies; it uses the `pickett-io` catalog and `.lin` types. The C code is only needed for the predictions and fits, which come from the `pickett` crate.

```rust
use pickett_spectrum::{assign::*, baseline::*, fit::*, peaks::*, Spectrum};

let spectrum = Spectrum::parse(&std::fs::read_to_string("scan.txt")?)?;
let flat = spectrum.minus(&asls(&spectrum.intensity, &AslsOptions { lambda: 1e8, p: 0.002, iterations: 15 }));
let peaks = find_peaks(&flat, &PeakOptions { min_snr: 6.0, min_distance: 3, ..Default::default() });
let fits: Vec<_> = peaks.iter().filter_map(|p| fit_peak(&flat, p, &FitOptions::default())).collect();
let centres: Vec<f64> = fits.iter().map(|f| f.center).collect();
let prediction = pickett::predict(pickett::Flavor::Spinv, &par, &int)?;
let m: Vec<_> = match_lines(&centres, &prediction.catalog.lines, &MatchOptions { tolerance: 1.0, ..Default::default() })
    .into_iter().filter(|a| !a.ambiguous).collect();
let errs: Vec<f64> = m.iter().map(|a| fits[a.observed].center_err).collect();
let lin = to_lin(&m, &prediction.catalog.lines, 0.0, Some(&errs));   // -> SPFIT
```

| Module | Contents |
|---|---|
| `Spectrum` | Reads frequency/intensity text: spaces, tabs, commas or semicolons; any two columns; comment and header lines skipped; sorted by frequency. Also `slice`, `nearest`, `minus`, `to_text` |
| `smooth::savitzky_golay` | Savitzky–Golay smoothing. At the ends, the first/last full window's polynomial is used, so no padding is made up |
| `baseline::asls` | Asymmetric least-squares baseline (Eilers & Boelens 2005), solved in O(n) with a pentadiagonal `L D Lᵀ` factorisation |
| `peaks::noise_level`, `find_peaks` | Robust noise (MAD of point differences). Peaks above an SNR, height or prominence, at least `min_distance` points from a higher peak, with sub-point parabolic centres and interpolated FWHM |
| `profile::Profile` | Gaussian, Lorentzian, Voigt (Thompson–Cox–Hastings pseudo-Voigt, within ~1% of the true Voigt); peak-normalised; `fwhm()`, `area()` |
| `fit::fit_peak` | Levenberg–Marquardt fit of one line plus a constant/linear local baseline. Centre fitted relative to the start for conditioning; standard errors scaled by the reduced χ² |
| `assign::match_lines`, `to_lin` | Closest-first unique matching within a tolerance, skipping lines weaker than `min_lgint`. `ambiguous` flags measurements with another predicted line within the tolerance; identical predicted frequencies count as one line. The matches are written as a `.lin` file with the predicted quanta |
| `synth` | `grid`, `synthesize`, and `from_catalog` (a spectrum from a `.cat`, with heights `10^LGINT / area`) |

**Assignment loop.** Starting constants are usually off, so assign in passes and refit after each one:
1. Start with the strong lines and a wide tolerance.
2. Narrow the tolerance and include weaker lines with each pass.
3. In the early passes, give the `.lin` errors a floor that matches the tolerance. SPFIT rejects lines with |obs−calc|/err above `ERRTST` (1000 in the golden `.par`), so with the true measured errors (~10⁻³ MHz) every line would be rejected while the constants are still tens of MHz off.
4. Use the measured errors only in the last pass, so the fitted errors are meaningful.

**Closed-loop test** (`tests/closed_loop.rs`): the whole chain on a simulated spectrum.
- **Simulated spectrum:** SPCAT lines from the SO₂-like golden constants, drawn as Gaussians (FWHM 1 MHz, 0.2 MHz steps, 50–350 GHz, 1.5 M points) on a sloping, wavy baseline, plus Gaussian noise (σ = 2×10⁻⁷).
- **Analysis:** AsLS baseline, peaks at SNR ≥ 8, Gaussian line fits.
- **Starting point:** the perturbed constants of `spfit_asym/input/asym.par` (A off by 8.5 MHz).
- **Passes:** 4, at 60 / 10 / 2 / 0.5 MHz.
- **Checks and results:**
  - the noise estimate is within 20%;
  - the fitted centres agree with the true frequencies (RMS pull < 2);
  - all 148 found lines are assigned, with no wrong quanta (3 wrong in pass 1, none after pass 2);
  - every fitted constant is within 5σ of the truth: A = 60778.54998 ± 0.00003 against the true 60778.55000;
  - the final weighted RMS is 0.99.

**Examples:**

```sh
cd rust
# a simulated measurement (1.5 M points) from the SO2-like golden constants
cargo run --release -p pickett-spectrum --example simulate -- ../test/golden/spcat_asym/input/asym.var ../test/golden/spcat_asym/input/asym.int so2.txt
# one assignment pass, then fit; repeat with the new .par and a smaller --tol
cargo run --release -p pickett-spectrum --example analyze -- so2.txt ../test/golden/spfit_asym/input/asym.par ../test/golden/spcat_asym/input/asym.int pass1.lin --tol 60 --min-lgint -4 --err-floor 3 --fit
cargo run --release -p pickett-spectrum --example analyze -- so2.txt pass1.par ../test/golden/spcat_asym/input/asym.int pass2.lin --tol 10 --min-lgint -5.5 --err-floor 0.5 --fit
```

Four passes (60 / 10 / 2 / 0.5 MHz) assigned all 149 predicted lines in range and gave A = 60778.550004 ± 0.00003.

Limits:
- Savitzky–Golay and the parabolic peak centres assume evenly spaced points.
- `fit_peak` fits one line at a time, so blended lines need a wider model, which is not provided.
- The pseudo-Voigt is an approximation (~1%), which matters for line widths more than for centres.
- `match_lines` uses only frequencies, not intensities.

---

## Legacy, alternate and unused files

| File | Status |
|---|---|
| [spinv0.c](spinv0.c) | Older SPINV (last revision 12 Dec 2005). [spinv.c](spinv.c) is the current one (adds the 10 Sept 2006 phase conventions). Not built. |
| [calfitno.c](calfitno.c) | Older variant of `calfit.c` (pre-2001 iteration control). Not built. |
| [dpix.c](dpix.c) | Variant of `dpi.c` with a different matrix-element pair table. Not built. |
| [slibgen.c](slibgen.c) | Generic (non-gcc) version of the system library. `slibgcc.c` is the one used. |
| [blas.c](blas.c), [blas.h](blas.h) | Pre-CBLAS BLAS subset. Replaced by `dblas.c`/`cblas.h`. |
| [jelim.c](jelim.c), [util.c](util.c), [fdate.c](fdate.c), [testi.c](testi.c) | Small helpers and tests. `testi.c` prints C type sizes. Not built. |
| [makefile.linux](makefile.linux) | Obsolete g77 makefile that refers to Fortran sources not present here (`calbak.f`, `cnvwn.f`, `slib.for`). |
| `splib.a` | Build output (the static library built from the shared sources). It used to be committed as a pre-built binary and is now git-ignored. |

---

## Known issues and caveats

1. **Partition-function grid edge case.** If the `.int` `TEMP` is exactly 0, the 0 K grid point remains and causes a division by zero. `TEMP = 0` is not physically valid for SPCAT anyway.
2. **Legacy C.** K&R definitions, `gets()` in the catalog utilities, and implicit `int main()` all trigger warnings. gcc 15+ (C23 by default) needs `-std=gnu17`.
3. **The PDF manuals' text layer is font-mangled** (ligatures and Greek letters encoded oddly). Read them in a PDF viewer rather than extracting the text.
4. **Library limits.** Memory is not freed after a failure in the middle of a calculation, and memory files go through temporary files (see [Library entry points](#library-entry-points-picketth)). Runs are parallel across threads. Within one run, only the partition-function table uses several cores.

### Fixed build issues

These were present in the 2008/2020 code and have been fixed:

| Issue | Fix |
|---|---|
| `iamcalc.c` included the missing `ftran.h` | Added [ftran.h](ftran.h) with the `ftran()` prototype |
| `iamcalc` link rule left out `readopt.o`; `iambak`, `reassign` and `sortegy` were missing | Added them to the [Makefile](Makefile) and to `make all` |
| `cnvwn` target pointed to a non-existent `cnvwn.c`, and the `calbak` rule was defined twice | Removed the dead target and the duplicate rule |
| `ifndef ($(BLASLIB))` always linked the bundled BLAS | Replaced it with `ifeq ($(strip $(BLASLIB)),)` |
| `calmrg.c` defined its own `unlink()`, which clashes with the C library on MinGW/Windows | Renamed it to `exptunlink()` (file-local) |
| Pre-built `splib.a` was committed, and `.gitignore` only covered `*.o` | Stopped tracking `splib.a`; `.gitignore` now covers `*.a`, `*.exe` and the built programs; added `make clean` |
| On Windows, `tmpfile()` (used for large-catalog sorting and scratch buffers) writes to the drive root and fails for ordinary users | `pk_tmpfile()` in [pktmp.c](pktmp.c), using the user's temporary folder |

---

## References

* H. M. Pickett, "The Fitting and Prediction of Vibration-Rotation Spectra with Spin Interactions," *J. Mol. Spectrosc.* **148**, 371–377 (1991).
* H. M. Pickett, "Spin eigenfunctions and operators for the Dn groups," *J. Mol. Spectrosc.* **228**, 659–663 (2004). (Itot basis)
* H. M. Pickett, "Use of Euler series to fit spectra with application to water," *J. Mol. Spectrosc.* **233**, 174–179 (2005).
* J. E. Dennis & R. B. Schnabel, *Numerical Methods for Unconstrained Optimization and Nonlinear Equations*, Prentice-Hall (1983). (Trust-region Marquardt)
* Bundled manuals: [calpgm.pdf](calpgm.pdf), [spinv.pdf](spinv.pdf), [dpi.pdf](dpi.pdf)
* Thai user manual: [MANUAL_TH.md](MANUAL_TH.md)
* CDMS Pickett program page: <https://cdms.astro.uni-koeln.de/classic/predictions/pickett/quelle/>
