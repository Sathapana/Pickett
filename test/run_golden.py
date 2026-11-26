#!/usr/bin/env python3
"""Golden-output regression tests for the CALPGM programs (SPFIT, SPCAT,
DPFIT, DPCAT, calbak).

    python test/run_golden.py            # build with make, run every case
    python test/run_golden.py -k spfit   # only cases whose name contains "spfit"
    python test/run_golden.py --no-build # use the executables already built
    python test/run_golden.py --update   # rewrite expected/ from this build

Each case lives in test/golden/<case>/:

    case.json   which program(s) to run, and optional compare overrides
    input/      files copied into a fresh temporary directory
    expected/   known-good outputs; every file here is compared

case.json keys:

    "steps":   [{"program": "spfit", "args": ["mol"]}, ...]   (required)
    "compare": {"mol.str": "numeric-abs", ...}  per-file mode override
    "rtol":    relative tolerance for "numeric" files (default 1e-6)
    "description": free text

Compare modes:

    exact        lines must be identical (only CRLF/LF is ignored).
                 Default for .cat and .egy.
    numeric      default for every other file. Timestamps are masked,
                 3-digit exponents (E+005, Windows) become 2-digit (E+05),
                 whitespace is ignored, integers must match exactly and
                 floats must agree within
                     max(rtol * |x|, one unit in the last printed digit).
    numeric-abs  like numeric, but compares |x| (for values whose sign is
                 an arbitrary eigenvector phase, e.g. dipoles in .str).

Python 3.6+ standard library only. Exit status 0 if every file passes.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "test", "golden")

DEFAULT_MODE = {".cat": "exact", ".egy": "exact"}
DEFAULT_RTOL = 1e-6

# e.g. "Thu Nov 26 02:23:13 2026" (ctime format, written by fdate/ctime)
TIMESTAMP_RE = re.compile(
    r"(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun) "
    r"(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) "
    r"[ \d]\d \d\d:\d\d:\d\d \d{4}")
# MSVCRT printf writes 3-digit exponents: 1.0E+005 -> 1.0E+05
EXP3_RE = re.compile(r"(?<=[0-9.])([EeDd][-+])0(\d\d)(?!\d)")
NUMBER_RE = re.compile(r"[-+]?(?:\d+\.\d*|\.\d+|\d+)(?:[EeDd][-+]?\d+)?")


# ---------------------------------------------------------------- compare

def read_lines(path):
    with open(path, "r", encoding="latin-1", newline=None) as f:
        return [ln.rstrip("\r\n") for ln in f.read().split("\n")]


def normalize(line):
    line = TIMESTAMP_RE.sub("<TIMESTAMP>", line)
    return EXP3_RE.sub(r"\1\2", line)


class Num(object):
    __slots__ = ("text", "value", "is_int", "ulp")

    def __init__(self, text):
        self.text = text
        t = text.replace("D", "E").replace("d", "e")
        self.value = float(t)
        mant, _, exp = t.lower().partition("e")
        self.is_int = "." not in mant and not exp
        decimals = len(mant.split(".", 1)[1]) if "." in mant else 0
        self.ulp = 10.0 ** (int(exp or 0) - decimals)


def tokenize(line):
    """Split a line into Num tokens and whitespace-free text tokens."""
    toks, pos = [], 0
    for m in NUMBER_RE.finditer(line):
        toks.extend(line[pos:m.start()].split())
        toks.append(Num(m.group()))
        pos = m.end()
    toks.extend(line[pos:].split())
    return toks


def numbers_differ(a, b, rtol, use_abs):
    if a.is_int and b.is_int:
        return abs(a.value) != abs(b.value) if use_abs else a.value != b.value
    x, y = (abs(a.value), abs(b.value)) if use_abs else (a.value, b.value)
    tol = max(rtol * max(abs(x), abs(y)), max(a.ulp, b.ulp) * 1.0000001)
    return abs(x - y) > tol


def compare_file(expected, actual, mode, rtol):
    """Return None if the files match, else a short description."""
    exp_lines, act_lines = read_lines(expected), read_lines(actual)
    # a trailing newline gives one empty last element; ignore trailing blanks
    while exp_lines and not exp_lines[-1].strip():
        exp_lines.pop()
    while act_lines and not act_lines[-1].strip():
        act_lines.pop()
    if len(exp_lines) != len(act_lines):
        return "line count %d, expected %d" % (len(act_lines), len(exp_lines))
    for n, (e, a) in enumerate(zip(exp_lines, act_lines), 1):
        if mode == "exact":
            if e != a:
                return "line %d differs\n    expected: %s\n    actual:   %s" % (n, e, a)
            continue
        if e == a:
            continue
        te, ta = tokenize(normalize(e)), tokenize(normalize(a))
        bad = len(te) != len(ta)
        if not bad:
            for x, y in zip(te, ta):
                if isinstance(x, Num) and isinstance(y, Num):
                    if numbers_differ(x, y, rtol, mode == "numeric-abs"):
                        bad = True
                        break
                elif isinstance(x, Num) or isinstance(y, Num) or x != y:
                    bad = True
                    break
        if bad:
            return "line %d differs\n    expected: %s\n    actual:   %s" % (n, e, a)
    return None


# ---------------------------------------------------------------- build/run

def find_make(user_make):
    if user_make:
        return user_make
    for cand in (os.environ.get("MAKE"), "make", "mingw32-make", "gmake"):
        if cand and shutil.which(cand):
            return cand
    sys.exit("error: no make found (tried make, mingw32-make, gmake); use --make")


def build(make, programs, clean, make_args, verbose):
    cmds = []
    if clean:
        cmds.append([make, "clean"])
    cmds.append([make] + list(make_args) + sorted(programs))
    for cmd in cmds:
        print("build: %s" % " ".join(cmd))
        p = subprocess.run(cmd, cwd=REPO, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, universal_newlines=True)
        if verbose or p.returncode != 0:
            print(p.stdout)
        if p.returncode != 0:
            sys.exit("error: build failed (%s)" % " ".join(cmd))


def program_path(name, bindir):
    for cand in (name + ".exe", name):
        path = os.path.join(bindir, cand)
        if os.path.isfile(path):
            return path
    return None


def load_cases(golden, patterns):
    cases = []
    for name in sorted(os.listdir(golden)):
        cfg = os.path.join(golden, name, "case.json")
        if not os.path.isfile(cfg):
            continue
        if patterns and not any(p in name for p in patterns):
            continue
        with open(cfg) as f:
            spec = json.load(f)
        spec["name"] = name
        spec["dir"] = os.path.join(golden, name)
        cases.append(spec)
    return cases


def run_case(case, bindir, args):
    """Run one case; return list of (file, ok, message)."""
    results = []
    work = tempfile.mkdtemp(prefix="pickett_%s_" % case["name"])
    indir = os.path.join(case["dir"], "input")
    for fn in sorted(os.listdir(indir)):
        shutil.copy(os.path.join(indir, fn), work)

    for i, step in enumerate(case["steps"]):
        exe = program_path(step["program"], bindir)
        if exe is None:
            results.append((step["program"], False, "executable not found in %s" % bindir))
            return results, work
        cmd = [exe] + step.get("args", [])
        log = os.path.join(work, "_step%d_%s.log" % (i + 1, step["program"]))
        with open(log, "w") as lf:
            try:
                p = subprocess.run(cmd, cwd=work, stdin=subprocess.DEVNULL,
                                   stdout=lf, stderr=subprocess.STDOUT,
                                   timeout=args.timeout)
                rc = p.returncode
            except subprocess.TimeoutExpired:
                rc = "timeout"
        want = step.get("returncode", 0)
        if rc != want:
            results.append((step["program"], False,
                            "exit status %s, expected %s (log: %s)" % (rc, want, log)))
            return results, work

    expdir = os.path.join(case["dir"], "expected")
    if args.update:
        os.makedirs(expdir, exist_ok=True)
        for fn in case.get("outputs") or sorted(os.listdir(expdir)):
            src = os.path.join(work, fn)
            if os.path.isfile(src):
                shutil.copy(src, os.path.join(expdir, fn))
                results.append((fn, True, "updated"))
            else:
                results.append((fn, False, "not produced, cannot update"))
        return results, work

    overrides = case.get("compare", {})
    rtol = case.get("rtol", args.rtol)
    for fn in sorted(os.listdir(expdir)):
        actual = os.path.join(work, fn)
        mode = overrides.get(fn, DEFAULT_MODE.get(os.path.splitext(fn)[1].lower(), "numeric"))
        if not os.path.isfile(actual):
            results.append((fn, False, "not produced"))
            continue
        msg = compare_file(os.path.join(expdir, fn), actual, mode, rtol)
        results.append((fn, msg is None, mode if msg is None else "%s: %s" % (mode, msg)))
    return results, work


def main():
    ap = argparse.ArgumentParser(
        description="Run the golden-output regression tests.",
        epilog="Example: python test/run_golden.py --clean --make-arg CFLAGS=-O0")
    ap.add_argument("-k", dest="patterns", action="append", default=[],
                    metavar="TEXT", help="only run cases whose name contains TEXT (repeatable)")
    ap.add_argument("--no-build", action="store_true", help="do not run make")
    ap.add_argument("--clean", action="store_true", help="run 'make clean' before building")
    ap.add_argument("--make", help="make command (default: make, mingw32-make or gmake)")
    ap.add_argument("--make-arg", action="append", default=[], metavar="ARG",
                    help="extra make argument, e.g. CFLAGS=-O0 (repeatable)")
    ap.add_argument("--bindir", default=REPO,
                    help="directory holding the executables (default: repo root)")
    ap.add_argument("--golden", default=GOLDEN,
                    help="directory holding the cases (default: test/golden)")
    ap.add_argument("--rtol", type=float, default=DEFAULT_RTOL,
                    help="relative tolerance for numeric files (default %g)" % DEFAULT_RTOL)
    ap.add_argument("--timeout", type=float, default=600, help="seconds per program run")
    ap.add_argument("--keep", action="store_true", help="keep the temporary run directories")
    ap.add_argument("--update", action="store_true",
                    help="copy this build's outputs into expected/ instead of comparing")
    ap.add_argument("-v", "--verbose", action="store_true", help="show make output")
    args = ap.parse_args()

    cases = load_cases(args.golden, args.patterns)
    if not cases:
        sys.exit("error: no cases found in %s" % args.golden)
    if not args.no_build:
        programs = {s["program"] for c in cases for s in c["steps"]}
        build(find_make(args.make), programs, args.clean, args.make_arg, args.verbose)

    nfail = ntotal = 0
    for case in cases:
        results, work = run_case(case, os.path.abspath(args.bindir), args)
        failed = any(not ok for _, ok, _ in results)
        for fn, ok, msg in results:
            ntotal += 1
            nfail += not ok
            print("%s  %s/%s  (%s)" % ("PASS" if ok else "FAIL", case["name"], fn, msg))
        if failed or args.keep:
            print("      run directory kept: %s" % work)
        else:
            shutil.rmtree(work, ignore_errors=True)

    print("\n%d of %d files passed, %d failed (%d cases)"
          % (ntotal - nfail, ntotal, nfail, len(cases)))
    return 1 if nfail else 0


if __name__ == "__main__":
    sys.exit(main())
