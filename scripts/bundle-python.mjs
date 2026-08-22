// Bundle a portable Python + mineru runtime into the installer.
//
// This script runs at BUILD TIME (before `pnpm tauri build`). It downloads a
// python-build-standalone "install_only" tarball, extracts it to
// `bundle_extras/python/`, and installs the `mineru` package into it via pip so
// that the shipped application needs zero network/setup on the user's machine.
//
// The layout mirrors what `src/engine/model_manager.rs::setup_python_environment`
// produces at runtime, so the bundled Python is picked up automatically by the
// app (install_dir/python/python.exe).

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const bundleExtras = path.join(root, "bundle_extras");
const pythonDir = path.join(bundleExtras, "python");
const pythonExe = path.join(pythonDir, "python.exe");

// Pinned to match the runtime download URL in model_manager.rs. If this exact
// asset ever disappears, `resolvePythonUrl()` falls back to the GitHub API.
const PYTHON_TARBALL_URL =
  "https://github.com/astral-sh/python-build-standalone/releases/download/20240713/cpython-3.12.4+20240713-x86_64-pc-windows-msvc-install_only.tar.gz";

// Resolve the Windows x86_64 install_only tarball for Python 3.12 from the
// given release tag via the GitHub API (used as a fallback when the pinned URL
// is unavailable).
async function resolvePythonUrlFromApi(tag = "20240713") {
  log("Pinned Python URL unavailable — querying GitHub API for a fallback…");
  const api = `https://api.github.com/repos/astral-sh/python-build-standalone/releases/tags/${tag}`;
  const res = await fetch(api, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  const data = await res.json();
  const asset = (data.assets || []).find(
    (a) =>
      /^cpython-3\.12\.\d+\+\d+-x86_64-pc-windows-msvc-install_only\.tar\.gz$/.test(
        a.name
      )
  );
  if (!asset) throw new Error("No Python 3.12 install_only asset found");
  return asset.browser_download_url;
}

const log = (msg) => console.log(`[bundle-python] ${msg}`);
const fail = (msg) => {
  console.error(`[bundle-python] ERROR: ${msg}`);
  process.exit(1);
};

function run(cmd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: "inherit", ...opts });
    child.on("error", reject);
    child.on("close", (code) =>
      code === 0 ? resolve(code) : reject(new Error(`${cmd} exited with ${code}`))
    );
  });
}

async function download(url, dest) {
  log(`Downloading ${url}`);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 10 * 60 * 1000); // 10 min
  let res;
  try {
    res = await fetch(url, { signal: controller.signal });
  } catch (e) {
    clearTimeout(timer);
    if (e.name === "AbortError") fail(`Download timed out after 10 min: ${url}`);
    fail(`Network error downloading ${url}: ${e.message}`);
  }
  clearTimeout(timer);

  // Fallback: pinned URL gone → ask the GitHub API for a matching asset.
  if (!res.ok) {
    try {
      const fallback = await resolvePythonUrlFromApi();
      log(`Falling back to ${fallback}`);
      const c2 = new AbortController();
      const t2 = setTimeout(() => c2.abort(), 10 * 60 * 1000);
      try {
        res = await fetch(fallback, { signal: c2.signal });
      } finally {
        clearTimeout(t2);
      }
    } catch (e) {
      fail(`HTTP ${res.status} downloading ${url} (${e.message})`);
    }
  }
  if (!res.ok) fail(`HTTP ${res.status} downloading ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  fs.writeFileSync(dest, buf);
  log(`Saved ${Math.round(buf.length / 1e6)} MB to ${dest}`);
}

// Recursively move the contents of `src` into `dest`, then remove `src`.
function moveDirContents(src, dest) {
  if (!fs.existsSync(src)) return;
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);
    if (fs.existsSync(to)) {
      if (entry.isDirectory()) moveDirContents(from, to);
      else fs.rmSync(to, { force: true });
    }
    fs.renameSync(from, to);
  }
  fs.rmSync(src, { recursive: true, force: true });
}

async function pipListHasMineru() {
  if (!fs.existsSync(pythonExe)) return false;
  try {
    const out = fs
      .readFileSync(
        await new Promise((resolve, reject) => {
          const child = spawn(
            pythonExe,
            ["-m", "pip", "list", "--format=columns"],
            { windowsHide: true }
          );
          const chunks = [];
          child.stdout.on("data", (d) => chunks.push(d));
          child.stderr.on("data", () => {});
          child.on("error", reject);
          child.on("close", (c) => {
            const tmp = path.join(bundleExtras, ".piplist.txt");
            fs.writeFileSync(tmp, Buffer.concat(chunks));
            c === 0 ? resolve(tmp) : reject(new Error("pip list failed"));
          });
        })
      )
      .toString();
    return /mineru/i.test(out);
  } catch {
    return false;
  }
}

async function main() {
  const force = process.argv.includes("--force");
  fs.mkdirSync(bundleExtras, { recursive: true });

  // 1. Ensure Python is extracted.
  if (fs.existsSync(pythonExe) && !force) {
    log("Bundled python.exe already present, skipping download.");
  } else {
    if (fs.existsSync(pythonDir)) fs.rmSync(pythonDir, { recursive: true, force: true });
    const tarball = path.join(bundleExtras, "python-download.tar.gz");
    await download(PYTHON_TARBALL_URL, tarball);

    log("Extracting Python runtime…");
    // python-build-standalone install_only tarballs have a top-level `python/`
    // directory, sometimes with an inner `install/` subdir — mirror the runtime
    // logic by extracting to bundle_extras then flattening `python/install`.
    await run("tar", ["-xzf", tarball, "-C", bundleExtras]);
    const installSub = path.join(pythonDir, "install");
    if (fs.existsSync(installSub)) {
      log("Flattening python/install → python/");
      moveDirContents(installSub, pythonDir);
    }
    fs.rmSync(tarball, { force: true });

    if (!fs.existsSync(pythonExe)) fail("python.exe not found after extraction");
    log("Python runtime extracted.");
  }

  // 2. Ensure mineru is installed into the bundled Python.
  if (await pipListHasMineru()) {
    log("mineru already installed in bundled Python, skipping pip install.");
  } else {
    log("Installing mineru into bundled Python (this may take a while)…");
    try {
      await run(pythonExe, ["-m", "pip", "install", "--upgrade", "mineru==3.4.5"], {
        windowsHide: true,
      });
    } catch {
      fail("pip install mineru failed (network required at build time)");
    }
    if (!(await pipListHasMineru())) fail("mineru not importable after install");
    log("mineru installed.");
  }

  log(`Done. Bundled runtime at: ${pythonDir}`);
}

main().catch((e) => fail(e.message || String(e)));
