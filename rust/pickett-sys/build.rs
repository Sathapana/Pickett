//! Compiles the CALPGM C sources from the repository root into one static
//! library, `libpickett.a`, with no build dependencies: it calls gcc and ar
//! directly.
//!
//! Both Hamiltonian variants go into the same library:
//! * SPCAT/SPFIT: calcat.c + calfit.c + subfit.c + spinv.c + spinit.c, with
//!   the run functions renamed to `pk_spcat_run` / `pk_spfit_run`;
//! * DPCAT/DPFIT: the same drivers + dpi.c, with every global symbol of those
//!   files prefixed `dp_` and the run functions named `pk_dpcat_run` /
//!   `pk_dpfit_run`;
//! * shared: ulib.c, cnjj.c, slibgcc.c, pktmp.c, catutil.c, lsqfit.c, dblas.c, sortsub.c.
//!
//! Thread-local state (see `pktls.h`) is on; set `PICKETT_NO_TLS=1` to build
//! without it, and runs are then serialized with a lock.
//!
//! Compiler choice: `PICKETT_CC`, then `CC`, then (on Windows)
//! `C:\msys64\mingw64\bin\gcc.exe`, then `gcc`. The first one whose
//! `-dumpmachine` matches the target architecture is used.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const COMMON: &[&str] = &["ulib", "cnjj", "slibgcc", "pktmp", "catutil", "lsqfit", "dblas", "sortsub"];
const DRIVERS: &[&str] = &["calcat", "calfit", "subfit"];
const SPINV: &[&str] = &["spinv", "spinit"];
const DPI: &[&str] = &["dpi"];
const HEADERS: &[&str] = &[
    "calpgm.h", "slib.h", "pickett.h", "pktls.h", "cblas.h", "cnjj.h", "lsqfit.h", "catutil.h", "spinit.h",
];

/// Global symbols defined in calcat.c, calfit.c, subfit.c and dpi.c that
/// also exist in the SPCAT/SPFIT variant (found with `nm -g`).
const DP_RENAMES: &[&str] = &[
    "ibufof", "qnfmt", "sblk_alloc", "simt", // calcat.c
    "linein", "lineix", "parer", "qnfmt2", // calfit.c
    "dnuadd", "dnuget", "filbak", "frqdat", "getblk", "getdbk", "getlbl", "lbufof", "lnlink",
    "prcorr", // subfit.c
    "getqn", "hamx", "intens", "setblk", "setfmt", "setint", "setopt", // dpi.c
];

struct Tool {
    cc: PathBuf,
    ar: PathBuf,
    path_env: Option<std::ffi::OsString>,
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let src = strip_verbatim(manifest.join("../..").canonicalize().expect("repository root"));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rerun-if-env-changed=PICKETT_CC");
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=PICKETT_NO_TLS");
    println!("cargo:rerun-if-changed=build.rs");
    for f in COMMON.iter().chain(DRIVERS).chain(SPINV).chain(DPI) {
        println!("cargo:rerun-if-changed={}", src.join(format!("{f}.c")).display());
    }
    let no_tls = env::var_os("PICKETT_NO_TLS").is_some();
    for h in HEADERS {
        println!("cargo:rerun-if-changed={}", src.join(h).display());
    }

    if target_env == "msvc" {
        panic!("pickett-sys needs a GNU toolchain (x86_64-pc-windows-gnu on Windows); MSVC is not supported");
    }
    let tool = find_tool(&arch, &target_os);

    let mut objects = Vec::new();
    let mut compile = |name: &str, tag: &str, defines: &[String]| {
        let obj = out.join(format!("{name}_{tag}.o"));
        let mut cmd = Command::new(&tool.cc);
        cmd.args(["-std=gnu17", "-O2", "-w", "-c"])
            .args(no_tls.then_some("-DPK_NO_TLS"))
            .arg(src.join(format!("{name}.c")))
            .arg("-I")
            .arg(&src)
            .args(defines)
            .arg("-o")
            .arg(&obj);
        run(&mut cmd, &tool);
        objects.push(obj);
    };

    for f in COMMON {
        compile(f, "common", &[]);
    }
    let sp = vec!["-Dcalcat_run=pk_spcat_run".to_string(), "-Dcalfit_run=pk_spfit_run".to_string()];
    for f in DRIVERS.iter().chain(SPINV) {
        compile(f, "sp", &sp);
    }
    let mut dp = vec!["-Dcalcat_run=pk_dpcat_run".to_string(), "-Dcalfit_run=pk_dpfit_run".to_string()];
    dp.extend(DP_RENAMES.iter().map(|s| format!("-D{s}=dp_{s}")));
    for f in DRIVERS.iter().chain(DPI) {
        compile(f, "dp", &dp);
    }

    let lib = out.join("libpickett.a");
    let _ = std::fs::remove_file(&lib);
    let mut ar = Command::new(&tool.ar);
    ar.arg("crs").arg(&lib).args(&objects);
    run(&mut ar, &tool);

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=pickett");
    if target_os != "windows" {
        println!("cargo:rustc-link-lib=m");
    }
}

fn run(cmd: &mut Command, tool: &Tool) {
    if let Some(p) = &tool.path_env {
        cmd.env("PATH", p);
    }
    let status = cmd.status().unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    if !status.success() {
        panic!("command failed ({status}): {cmd:?}");
    }
}

/// Picks a C compiler for the target architecture, and the `ar` next to it.
fn find_tool(arch: &str, target_os: &str) -> Tool {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["PICKETT_CC", "CC"] {
        if let Some(v) = env::var_os(var) {
            candidates.push(PathBuf::from(v));
        }
    }
    if target_os == "windows" && arch == "x86_64" {
        candidates.push(PathBuf::from(r"C:\msys64\mingw64\bin\gcc.exe"));
    }
    candidates.push(PathBuf::from("gcc"));
    candidates.push(PathBuf::from("cc"));

    let want = match arch {
        "x86" => "i686",
        other => other,
    };
    let mut tried = Vec::new();
    for cc in candidates {
        let path_env = cc.parent().filter(|d| !d.as_os_str().is_empty()).map(|d| prepend_path(d));
        let mut cmd = Command::new(&cc);
        cmd.arg("-dumpmachine");
        if let Some(p) = &path_env {
            cmd.env("PATH", p);
        }
        let machine = match cmd.output() {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => {
                tried.push(format!("{} (not runnable)", cc.display()));
                continue;
            }
        };
        let machine_arch = machine.split('-').next().unwrap_or("");
        let matches = machine_arch == want
            || (want == "i686" && matches!(machine_arch, "i386" | "i486" | "i586" | "mingw32"));
        if !matches {
            tried.push(format!("{} ({machine})", cc.display()));
            continue;
        }
        let ar = cc
            .parent()
            .map(|d| d.join(if target_os == "windows" { "ar.exe" } else { "ar" }))
            .filter(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("ar"));
        return Tool { cc, ar, path_env };
    }
    panic!(
        "no C compiler for target arch {arch} found; tried: {}.\n\
         Set PICKETT_CC to a gcc for this target. On Windows (x86_64-pc-windows-gnu) install \
         MSYS2 and run: pacman -S mingw-w64-x86_64-gcc",
        tried.join(", ")
    );
}

/// `canonicalize` gives `\\?\C:\...` on Windows, which gcc cannot open.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) => PathBuf::from(rest),
        None => p,
    }
}

fn prepend_path(dir: &Path) -> std::ffi::OsString {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(old) = env::var_os("PATH") {
        paths.extend(env::split_paths(&old));
    }
    env::join_paths(paths).unwrap_or_default()
}
