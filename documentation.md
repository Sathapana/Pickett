# Pickett CALPGM Suite: Developer & User Documentation

This repository holds the C source for **Herbert M. Pickett's CALPGM program suite**, best known for **SPFIT** (fitting) and **SPCAT** (prediction/catalog generation). Spectroscopists use these programs to fit and simulate rotational, ro-vibrational and hyperfine spectra of molecules. The programs come from JPL, and the CDMS (Cologne) and JPL molecular spectroscopy catalogs are built with them.

This document covers how the code is organised, how to build and run it, the main file formats, and what this fork changes. The authoritative physics and input-format reference is still the set of PDF manuals that ship with the code (see [References](#references)).

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
10. [Legacy, alternate and unused files](#legacy-alternate-and-unused-files)
11. [Known issues and caveats](#known-issues-and-caveats)
12. [References](#references)

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
| [slibgcc.c](slibgcc.c), [slib.h](slib.h) | System-dependent code: command-line/interactive file-name handling (`filget`), `fopenq`, memory sizing, time stamps |
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
| [test/](test/) | CH₃CN prolate symmetric-top regression case (see [below](#test-case-and-verification)) |
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
gcc -O2 -o spfit calfit.c subfit.c spinv.c spinit.c $LIB -lm
gcc -O2 -o spcat calcat.c sortsub.c spinv.c spinit.c $LIB -lm
gcc -O2 -o dpfit calfit.c subfit.c dpi.c $LIB -lm
gcc -O2 -o dpcat calcat.c sortsub.c dpi.c $LIB -lm
gcc -O2 -o calmrg calmrg.c $LIB -lm
```

The code is K&R/ANSI C from ~1999–2008, so a modern compiler prints many warnings (implicit `int`, old-style definitions). Add `-w` to silence them. Recent gcc/clang releases that default to C23 may reject the K&R function definitions in some files. If that happens, add `-std=gnu99` (or `-std=gnu89`).

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

To change the range, edit `TMAX` and set `NTEMP = TMAX + 1`. The grid step is fixed at 1 K. Each extra temperature adds one `exp()` per energy level, so the cost grows with `NTEMP`, but it is small next to diagonalisation. **Note:** Q is only meaningful if the calculation includes all levels that are populated at the highest temperature. Make sure `FEND`, `KNMAX` and the vibrational states you include are large enough for high-T values.

The `QROT` value on the `.int` line still sets the `.cat` intensities. The tabulated Q values are diagnostic output only and are **not** fed back automatically.

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

There is no automated test harness. `test/` is the only regression reference, and it covers only SPCAT (not SPFIT, DPFIT or the utilities).

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
2. **Legacy C.** K&R definitions, `gets()` in the catalog utilities, and implicit `int main()` all trigger warnings. Very new compilers may need `-std=gnu99`.
3. **The PDF manuals' text layer is font-mangled** (ligatures and Greek letters encoded oddly). Read them in a PDF viewer rather than extracting the text.

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

---

## References

* H. M. Pickett, "The Fitting and Prediction of Vibration-Rotation Spectra with Spin Interactions," *J. Mol. Spectrosc.* **148**, 371–377 (1991).
* H. M. Pickett, "Spin eigenfunctions and operators for the Dn groups," *J. Mol. Spectrosc.* **228**, 659–663 (2004). (Itot basis)
* H. M. Pickett, "Use of Euler series to fit spectra with application to water," *J. Mol. Spectrosc.* **233**, 174–179 (2005).
* J. E. Dennis & R. B. Schnabel, *Numerical Methods for Unconstrained Optimization and Nonlinear Equations*, Prentice-Hall (1983). (Trust-region Marquardt)
* Bundled manuals: [calpgm.pdf](calpgm.pdf), [spinv.pdf](spinv.pdf), [dpi.pdf](dpi.pdf)
* CDMS Pickett program page: <https://cdms.astro.uni-koeln.de/classic/predictions/pickett/quelle/>
