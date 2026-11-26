"""Tests of the Python package on the golden cases of test/golden/.

Run from the repository root (after `maturin develop` in rust/pickett-py):
    python -m pytest rust/pickett-py/tests -q
"""

import pathlib
import threading
import time
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import pytest

import pickett
from pickett import spectrum as sp

GOLDEN = pathlib.Path(__file__).resolve().parents[3] / "test" / "golden"


def text(rel: str) -> str:
    return (GOLDEN / rel).read_text(encoding="latin-1").replace("\r\n", "\n")


CAT_CASES = [("spcat_prolate", "prolate", "spinv"), ("spcat_asym", "asym", "spinv"), ("dpcat_pi2", "pi2", "dpi")]
FIT_CASES = [("spfit_asym", "asym", "spinv"), ("spfit_prolate", "prolate", "spinv"), ("dpfit_pi2", "pi2", "dpi")]


@pytest.mark.parametrize("case,base,flavor", CAT_CASES)
def test_predict_reproduces_golden_catalog(case, base, flavor):
    p = pickett.predict(GOLDEN / case / "input" / f"{base}.var", GOLDEN / case / "input" / f"{base}.int", flavor)
    assert p.catalog.replace("\r\n", "\n") == text(f"{case}/expected/{base}.cat")
    f = p.lines["freq"]
    assert len(p) == len(f) == p.lines["upper"].shape[0] == p.lines["lower"].shape[0]
    assert np.all(np.diff(f) >= 0)
    assert p.partition.shape == (1000, 2) and p.partition[0, 0] == 1000.0
    assert p.log and p.report and f"pk.cat" in p.files


def test_text_and_path_inputs_agree_and_partition_function():
    a = pickett.predict(str(GOLDEN / "spcat_prolate/input/prolate.var"), str(GOLDEN / "spcat_prolate/input/prolate.int"))
    b = pickett.predict(text("spcat_prolate/input/prolate.var"), text("spcat_prolate/input/prolate.int"))
    assert np.array_equal(a.lines["freq"], b.lines["freq"])
    assert abs(a.partition_at(300.0) - 7820.3911) < 1e-3
    q = a.partition_at(300.5)
    assert a.partition_at(300.0) < q < a.partition_at(301.0)
    assert a.partition_at(0.5) is None and a.partition_at(5000.0) is None


@pytest.mark.parametrize("case,base,flavor", FIT_CASES)
def test_fit_reproduces_golden_parameters(case, base, flavor):
    r = pickett.fit(GOLDEN / case / "input" / f"{base}.par", GOLDEN / case / "input" / f"{base}.lin", flavor)
    expected = [float(l.split()[1]) for l in text(f"{case}/expected/{base}.var").splitlines()[3:]
                if len(l.split()) >= 3 and "/" in l]
    assert len(r.params["value"]) == len(expected)
    np.testing.assert_allclose(r.params["value"], expected, rtol=1e-6, atol=1e-12)
    assert r.rms_error is not None and len(r.iterations["iteration"]) >= 1
    assert "END OF ITERATION" in r.report
    # the fitted .var feeds a prediction
    if flavor == "spinv" and case == "spfit_asym":
        p = pickett.predict(r.var, GOLDEN / "spcat_asym/input/asym.int")
        assert len(p) == 198


def test_errors_carry_the_program_message():
    with pytest.raises(pickett.PickettError) as e:
        pickett.predict("title only\n", GOLDEN / "spcat_prolate/input/prolate.int")
    assert "Error reading .VAR file" in e.value.log
    with pytest.raises(ValueError):
        pickett.predict("x\n", "y\n", flavor="nonsense")


def test_pandas_frames():
    pd = pytest.importorskip("pandas")
    p = pickett.predict(GOLDEN / "spcat_asym/input/asym.var", GOLDEN / "spcat_asym/input/asym.int")
    df = p.lines_frame()
    assert isinstance(df, pd.DataFrame) and len(df) == 198
    assert {"freq", "lgint", "qn_up1", "qn_lo3"} <= set(df.columns)
    assert len(p.levels_frame()) == len(p.levels["energy"])
    r = pickett.fit(GOLDEN / "spfit_asym/input/asym.par", GOLDEN / "spfit_asym/input/asym.lin")
    assert list(r.params_frame()["label"])[:3] == ["A", "B", "C"]


def test_python_threads_run_in_parallel_with_identical_results():
    assert pickett.thread_safe()
    var, int_ = text("spcat_prolate/input/prolate.var"), text("spcat_prolate/input/prolate.int")
    ref = pickett.predict(var, int_, threads=1)
    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(lambda _: pickett.predict(var, int_, threads=1), range(4)))
    for r in results:
        assert np.array_equal(r.lines["freq"], ref.lines["freq"])
        assert np.array_equal(r.partition, ref.partition)


def test_gil_is_released_during_a_run():
    var, int_ = text("spcat_prolate/input/prolate.var"), text("spcat_prolate/input/prolate.int")

    def spin(n=300_000):
        t = time.perf_counter()
        s = 0
        for i in range(n):
            s += i
        return time.perf_counter() - t

    alone = min(spin() for _ in range(3))
    worker = threading.Thread(target=lambda: pickett.predict(var, int_, threads=1))
    t0 = time.perf_counter()
    worker.start()
    time.sleep(0.02)
    during = spin()
    worker.join()
    run = time.perf_counter() - t0
    # with the GIL held, the Python loop would wait for the whole run
    assert during < max(5 * alone, 0.5 * run), f"loop {during:.3f}s vs {alone:.3f}s alone (run {run:.3f}s)"


def test_predict_many_keeps_order():
    var, int_ = text("spcat_asym/input/asym.var"), text("spcat_asym/input/asym.int")
    jobs = []
    for k in range(6):
        lines = var.splitlines()
        for i, l in enumerate(lines):
            if l.strip().endswith("/A"):
                f = l.split()
                lines[i] = l.replace(f[1], f"{float(f[1]) + 10.0 * k:.15E}")
        jobs.append(("\n".join(lines) + "\n", int_))
    many = pickett.predict_many(jobs, threads=3)
    first = [m.lines["freq"][0] for m in many]
    single = [pickett.predict(v, i).lines["freq"][0] for v, i in jobs]
    assert first == single and len(set(first)) == 6


def test_spectrum_tools_basics():
    grid = np.arange(1000.0, 1010.0, 0.01)
    y = sp.synthesize([1003.0, 1007.5], [1.0, 0.5], grid, "gaussian", 0.1)
    pk = sp.find_peaks(grid, y, noise=1e-3)
    assert len(pk["freq"]) == 2 and abs(pk["freq"][0] - 1003.0) < 1e-3
    fit = sp.fit_peaks(grid, y, pk)
    assert fit["converged"].all() and np.allclose(fit["center"], [1003.0, 1007.5], atol=1e-6)
    assert np.allclose(sp.savgol(grid * 2.0, 4, 2), grid * 2.0)
    f, i = sp.read("100 1\n101,2\n# c\n102;3\n")
    assert list(f) == [100.0, 101.0, 102.0] and list(i) == [1.0, 2.0, 3.0]
    with pytest.raises(ValueError):
        sp.savgol(grid, 1, 5)
    assert abs(sp.profile_area("gaussian", 1.0) - 1.0644670194312262) < 1e-12


def test_closed_loop_in_python():
    """Simulated measurement -> peaks -> fits -> assignment -> SPFIT, from
    perturbed constants, recovers the true constants."""
    truth = pickett.predict(GOLDEN / "spcat_asym/input/asym.var", GOLDEN / "spcat_asym/input/asym.int")
    f, y = sp.catalog_spectrum(truth, 100_000.0, 250_000.0, 0.2, "gaussian", 1.0)
    rng = np.random.default_rng(7)
    y = y + 4e-6 + 2e-6 * np.sin(f / 9000.0) + 2e-7 * rng.standard_normal(len(f))
    flat = y - sp.asls(y, lam=1e8, p=0.002, iterations=15)
    pk = sp.find_peaks(f, flat, min_snr=8, min_distance=3)
    fits = sp.fit_peaks(f, flat, pk)
    ok = fits["converged"] & np.isfinite(fits["center_err"])
    centres, errs = fits["center"][ok], np.maximum(fits["center_err"][ok], 1e-4)

    par = text("spfit_asym/input/asym.par")
    int_ = text("spcat_asym/input/asym.int")
    for tol, min_lgint, floor in [(60, -4, 3.0), (10, -5.5, 0.5), (2, -7, 0.05), (0.5, -7, 0.0)]:
        pred = pickett.predict(par, int_)
        m = sp.match_lines(centres, pred, tolerance=tol, min_lgint=min_lgint)
        lin = sp.to_lin(pred, m, np.maximum(errs[m["observed"]], floor))
        result = pickett.fit(par, lin)
        par = result.par
    assert len(m["obs"]) >= 0.9 * len(centres)
    true_a = 60778.55
    a, a_err = result.value("A"), result.params["err_fit"][0]
    assert abs(a - true_a) < 5 * a_err and a_err < 1e-3, (a, a_err)
