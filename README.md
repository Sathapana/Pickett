# Pickett

## SPFIT/SPCAT program for rotational spectroscopy

This repository is used for a public, version controlled set of source code for Herb Pickett's SPFIT and SPCAT programs for fitting and simulating rotational spectra.

The base version of this code was obtained from the [Cologne Database for Molecular Spectroscopy](https://cdms.astro.uni-koeln.de/classic/predictions/pickett/quelle/), with the latest modifications by Dr. Holger S. P. Müller (the `Pickett_neu.zip` file package).

This repository includes modifications by Kelvin Lee, primarily with modifications to the partition function routine to calculate the rotational partition function up to 1000 K programmatically without needing to repetitively call SPCAT.


## Repository layout

| Folder | Contents |
|---|---|
| [src/](src/) | C sources of SPFIT, SPCAT, DPFIT, DPCAT and the auxiliary programs |
| [docs/](docs/) | Developer guide, Thai user manual, original PDF manuals, upstream changelog |
| [test/](test/) | Golden-output regression suite (`python test/run_golden.py`) and the original CH₃CN test files |
| [rust/](rust/) | Rust crates (file I/O, in-process runner, high-level API, spectrum analysis) and the Python package |
| [archive/](archive/) | Superseded and unused files from the original distribution |

Build the programs with `make` (or `mingw32-make` on Windows). They go to `bin/`, and intermediate files to `build/`.

## Documentation

- [documentation.md](docs/documentation.md): developer guide (architecture, building, file formats, known issues)
- [MANUAL_TH.md](docs/MANUAL_TH.md): user manual in Thai (installation, quick start, file formats, troubleshooting)
- [spinv.pdf](docs/spinv.pdf), [dpi.pdf](docs/dpi.pdf), [calpgm.pdf](docs/calpgm.pdf): original manuals by H. M. Pickett
- [archive/](archive/): superseded and unused files from the original distribution (see [archive/README.md](archive/README.md))
