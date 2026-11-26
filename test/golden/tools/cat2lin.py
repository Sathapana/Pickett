#!/usr/bin/env python3
"""Make a synthetic SPFIT/DPFIT .lin file from a subset of a .cat file.

This is how the inputs of the spfit_* / dpfit_* golden cases were made. It
is kept so the cases can be rebuilt; the harness itself does not run it.

Steps:
  1. Read the catalog (fixed JPL/CDMS format) and keep every N-th line
     (optionally only lines below a frequency limit).
  2. Add a small deterministic "measurement noise" to each frequency so
     the fit has non-zero residuals, set the experimental error, and make
     the species tag negative (calbak only converts lines with TAG < 0,
     i.e. lines flagged as laboratory measurements).
  3. Run calbak on that temporary catalog to write the .lin file.

Usage:
  python cat2lin.py CALBAK IN.cat OUT.lin [--every N] [--fmax MHZ]
                    [--err MHZ] [--noise MHZ]
"""
import argparse
import os
import subprocess
import sys
import tempfile


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("calbak", help="path to the calbak executable")
    ap.add_argument("cat", help="input catalog")
    ap.add_argument("lin", help="output .lin file")
    ap.add_argument("--every", type=int, default=40, help="keep every N-th line")
    ap.add_argument("--offset", type=int, default=0, help="index of the first line kept")
    ap.add_argument("--fmax", type=float, default=1e30, help="max frequency (MHz)")
    ap.add_argument("--err", type=float, default=0.05, help="experimental error (MHz)")
    ap.add_argument("--noise", type=float, default=0.02,
                    help="max size of the deterministic noise added (MHz)")
    args = ap.parse_args()

    with open(args.cat) as f:
        lines = [ln.rstrip("\r\n") for ln in f if len(ln.rstrip("\r\n")) > 55]
    lines = [ln for ln in lines if float(ln[0:13]) <= args.fmax]
    picked = lines[args.offset::args.every]

    out = []
    for i, ln in enumerate(picked):
        freq = float(ln[0:13])
        # deterministic pseudo-noise, uniform-ish in [-noise, +noise]
        noise = args.noise * (((i * 7919) % 201) - 100) / 100.0
        # columns: FREQ 0-13, ERR 13-21, LGINT..GUP 21-44, TAG 44-51, QNFMT+QN 51-
        tag = -abs(int(ln[44:51]))
        out.append("%13.4f%8.4f%s%7d%s" % (freq + noise, args.err, ln[21:44], tag, ln[51:]))

    fd, tmp = tempfile.mkstemp(suffix=".cat")
    os.close(fd)
    try:
        with open(tmp, "w") as f:
            f.write("\n".join(out) + "\n")
        subprocess.run([args.calbak, tmp, args.lin], check=True,
                       stdout=subprocess.DEVNULL)
    finally:
        os.remove(tmp)
    print("%d lines written to %s" % (len(picked), args.lin))


if __name__ == "__main__":
    sys.exit(main())
