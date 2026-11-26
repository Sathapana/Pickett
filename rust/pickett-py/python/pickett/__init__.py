"""Predict and fit rotational spectra with Pickett's SPCAT/SPFIT.

The programs run in memory: inputs are the texts of the usual files (or
paths to them), results are numpy arrays and plain Python objects. No files
are written and nothing is printed; the programs' messages are in ``.log``.

    import pickett
    fit = pickett.fit("mol.par", "mol.lin")
    print(fit.params_frame())
    pred = pickett.predict(fit.var, "mol.int")
    print(pred.lines["freq"][:5], pred.partition_at(300.0))

``flavor="spinv"`` runs SPCAT/SPFIT, ``flavor="dpi"`` DPCAT/DPFIT.

Each run releases the GIL, so Python threads (for example a
``concurrent.futures.ThreadPoolExecutor``) run predictions in parallel.
See ``pickett.spectrum`` for measured spectra.
"""

from __future__ import annotations

import os
from typing import Iterable, Optional, Sequence, Tuple, Union

import numpy as np

from . import _native
from ._native import PickettError, set_threads, thread_safe

__all__ = [
    "Prediction", "FitResult", "PickettError", "predict", "predict_many", "fit",
    "thread_safe", "set_threads", "read_text",
]

TextOrPath = Union[str, "os.PathLike[str]"]


def read_text(src: TextOrPath) -> str:
    """The text of `src`: a path (``os.PathLike``, or a ``str`` without a
    newline that names an existing file) is read; any other ``str`` is used
    as the text itself."""
    if isinstance(src, os.PathLike):
        with open(src, encoding="latin-1") as f:
            return f.read()
    if isinstance(src, str):
        if "\n" not in src and os.path.isfile(src):
            with open(src, encoding="latin-1") as f:
                return f.read()
        return src
    raise TypeError(f"expected file text or a path, not {type(src).__name__}")


def _frame(columns: dict):
    try:
        import pandas as pd
    except ImportError as e:  # pragma: no cover - depends on the environment
        raise ImportError("this needs pandas: pip install pandas") from e
    return pd.DataFrame(columns)


def _quanta_columns(prefix: str, q: np.ndarray) -> dict:
    return {f"{prefix}{k + 1}": q[:, k] for k in range(q.shape[1])} if q.size else {}


class Prediction:
    """The result of :func:`predict`.

    Attributes:
        lines: dict of arrays, one entry per catalog line, sorted by frequency:
            ``freq``, ``err`` (MHz, or cm⁻¹ in IR mode), ``lgint`` (log10 nm² MHz),
            ``strength`` (S·μ², D²), ``elow`` (cm⁻¹), ``gup``, ``dr``, ``tag``,
            ``qnfmt``, and ``upper``/``lower`` (2-D integer arrays of quanta).
            Values are full precision, not rounded like the ``.cat`` file.
        levels: dict of arrays: ``block``, ``index``, ``energy`` (cm⁻¹), ``err``,
            ``mixing``, ``degeneracy``, ``qn`` (2-D).
        partition: array of shape (n, 2): temperature (K) and Q, 1000 K → 1 K.
        catalog: the ``.cat`` file as SPCAT writes it (text).
        report: the ``.out`` report. log: the program's messages.
        files: every file of the run by name, as bytes.
    """

    def __init__(self, d: dict):
        self.lines = d["lines"]
        self.levels = d["levels"]
        self.partition = np.column_stack([d["partition_t"], d["partition_q"]])
        self.catalog: str = d["catalog"]
        self.report: str = d["report"]
        self.log: str = d["log"]
        self.files: dict = d["files"]

    def __len__(self) -> int:
        return len(self.lines["freq"])

    def __repr__(self) -> str:
        return f"<pickett.Prediction: {len(self)} lines, {len(self.levels['energy'])} levels>"

    def partition_at(self, temp: float) -> Optional[float]:
        """Q at `temp` K: exact at table points, otherwise ln Q interpolated
        linearly in ln T. None outside the table."""
        t, q = self.partition[:, 0], self.partition[:, 1]
        ok = (t > 0) & (q > 0)
        t, q = t[ok], q[ok]
        order = np.argsort(t)
        t, q = t[order], q[order]
        if len(t) == 0 or temp < t[0] or temp > t[-1]:
            return None
        exact = np.nonzero(t == temp)[0]
        if len(exact):
            return float(q[exact[0]])
        return float(np.exp(np.interp(np.log(temp), np.log(t), np.log(q))))

    def lines_frame(self):
        """The lines as a pandas DataFrame (needs pandas)."""
        cols = {k: v for k, v in self.lines.items() if k not in ("upper", "lower")}
        cols.update(_quanta_columns("qn_up", self.lines["upper"]))
        cols.update(_quanta_columns("qn_lo", self.lines["lower"]))
        return _frame(cols)

    def levels_frame(self):
        """The energy levels as a pandas DataFrame (needs pandas)."""
        cols = {k: v for k, v in self.levels.items() if k != "qn"}
        cols.update(_quanta_columns("qn", self.levels["qn"]))
        return _frame(cols)


class FitResult:
    """The result of :func:`fit`.

    Attributes:
        params: dict: ``id`` and ``label`` (lists of str), ``value``, ``err_fit``
            (fitted error) and ``err_par`` (error written to .par) as arrays.
        iterations: dict of arrays per iteration: ``iteration``,
            ``rms_error_old``, ``rms_error_new`` (weighted), ``rms_mw`` (MHz),
            ``rms_ir``, ``lines_used``, ``lines_rejected``.
        par: the new ``.par`` text (start of the next fit).
        var: the ``.var`` text (parameters + covariance; input to :func:`predict`).
        report: the ``.fit`` report. log: the program's messages.
    """

    def __init__(self, d: dict):
        self.params = d["params"]
        self.iterations = d["iterations"]
        self.par: str = d["par"]
        self.var: str = d["var"]
        self.report: str = d["report"]
        self.log: str = d["log"]

    def __repr__(self) -> str:
        return f"<pickett.FitResult: {len(self.params['value'])} parameters, rms {self.rms_error}>"

    @property
    def rms_error(self) -> Optional[float]:
        """Weighted RMS (obs−calc)/err after the last iteration."""
        r = self.iterations["rms_error_new"]
        return float(r[-1]) if len(r) else None

    def params_frame(self):
        """The parameters as a pandas DataFrame (needs pandas)."""
        return _frame(self.params)

    def value(self, label: str) -> float:
        """The fitted value of the parameter with this label (e.g. ``"A"``)."""
        return float(self.params["value"][self.params["label"].index(label)])


def predict(var: TextOrPath, int: TextOrPath, flavor: str = "spinv", threads: Optional[int] = None) -> Prediction:
    """SPCAT (``flavor="spinv"``) or DPCAT (``"dpi"``) on a ``.var`` and an
    ``.int`` (texts or paths). `threads`: cores this run may use for its
    partition-function table (default: all; results do not depend on it).
    Raises :class:`PickettError` (with ``.log``) when the program fails."""
    return Prediction(_native.predict(read_text(var), read_text(int), flavor, threads))


def predict_many(jobs: Iterable[Tuple[TextOrPath, TextOrPath]], flavor: str = "spinv",
                 threads: Optional[int] = None) -> list:
    """Many predictions on all cores (or `threads`); `jobs` are
    ``(var, int)`` pairs. Results are in job order."""
    texts = [(read_text(v), read_text(i)) for v, i in jobs]
    return [Prediction(d) for d in _native.predict_many(texts, flavor, threads)]


def fit(par: TextOrPath, lin: TextOrPath, flavor: str = "spinv") -> FitResult:
    """SPFIT (``flavor="spinv"``) or DPFIT (``"dpi"``) on a ``.par`` and a
    ``.lin`` (texts or paths). Raises :class:`PickettError` when the program
    fails or does not iterate."""
    return FitResult(_native.fit_lines(read_text(par), read_text(lin), flavor))
