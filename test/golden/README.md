# Golden-output regression cases

Run them all from the repository root:

```sh
python test/run_golden.py
```

The script builds the needed programs with `make` (or `mingw32-make`), runs each case in a temporary directory, and compares the outputs with `expected/`. See [documentation.md](../../documentation.md#regression-tests) and `python test/run_golden.py --help`.

## Layout

```
test/golden/<case>/
    case.json    programs to run, files to (re)generate, compare overrides
    input/       copied into the temporary run directory
    expected/    known-good outputs; every file here is compared
tools/cat2lin.py helper that made the synthetic .lin inputs
```

## Cases

| Case | Program | What it covers |
|---|---|---|
| `spcat_prolate` | SPCAT | CH₃CN prolate symmetric top, signed-K quanta, 1000→1 K partition-function table. Inputs and outputs are the original `test/prolate.*` files (2020, Linux reference), **not** regenerated. |
| `spfit_prolate` | SPFIT | Same molecule. B, D_JK, D_J start from perturbed values, with C, D_JK, D_J ties (negative IDPAR). 58 lines. |
| `spcat_asym` | SPCAT | SO₂-like asymmetric top (Watson A, 8 constants), b-type dipole, spin weights 1/0, N ≤ 20. Writes `.cat`, `.egy`, `.str`, `.out`. |
| `spfit_asym` | SPFIT | All 8 Watson A constants floated from perturbed values. 50 b-type lines. |
| `dpcat_pi2` | DPCAT | NO-like ²Π state in Hund's case (a), Λ-doubling (p, q), I = 1 magnetic and quadrupole hyperfine, F ≤ 12. |
| `dpfit_pi2` | DPFIT | Same state. A fixed, 9 constants floated. 68 hyperfine lines. |
| `calbak` | calbak | 20 merged-catalog lines (negative TAG) from the `spcat_asym` catalog converted to `.lin`. |

The constants for the asymmetric-top and ²Π cases are illustrative: close to SO₂ and NO, but they are not literature fits. They exist to exercise the code paths, not to give real spectra.

## How the fit inputs were made

Each SPFIT/DPFIT `.lin` is a subset of the matching SPCAT/DPCAT catalog. The catalog is the one from `spcat_prolate` for the prolate case, and the expected `.cat` of `spcat_asym` / `dpcat_pi2` for the others. Small deterministic noise is added so the residuals are not zero:

```sh
python test/golden/tools/cat2lin.py ./calbak test/golden/spcat_prolate/expected/prolate.cat prolate.lin --every 40 --offset 7
python test/golden/tools/cat2lin.py ./calbak test/golden/spcat_asym/expected/asym.cat asym.lin --every 4 --offset 1
python test/golden/tools/cat2lin.py ./calbak test/golden/dpcat_pi2/expected/pi2.cat pi2.lin --every 3 --err 0.02 --noise 0.01
```

The `.par` files are the SPCAT `.var` constants with some values perturbed and their errors raised, so that SPFIT/DPFIT has something to fit.

## Updating the expected outputs

Only do this when an output change is intended and has been checked:

```sh
python test/run_golden.py --update -k spfit_asym   # one case
python test/run_golden.py --update                 # every case (also overwrites spcat_prolate)
```

`--update` copies the files listed under `"outputs"` in `case.json` into `expected/`. Review the result with `git diff test/golden` before committing.
