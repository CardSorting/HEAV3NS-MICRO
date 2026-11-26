#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * Deterministic macOS Release Packager & Signer for HEAV3NS-CODE.app + heav3ns-micro.
 *
 * Usage:
 *   node bin/release-app.mjs [--skip-build] [--dmg] [--zip] [--install] [--notary-profile <name>] [--sign-id <identity>] [--dry-run]
 */
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execFileSync } from "node:child_process";

const argv = process.argv.slice(2);
if (argv.includes("--help") || argv.includes("-h")) {
  console.log("Usage: node bin/release-app.mjs [--skip-build] [--dmg] [--zip] [--install] [--notary-profile <name>] [--sign-id <id>] [--dry-run]");
  process.exit(0);
}
const hasFlag = (f) => argv.includes(f);
const flagVal = (f) => {
  const i = argv.indexOf(f);
  return i >= 0 && i + 1 < argv.length && !argv[i + 1].startsWith("--") ? argv[i + 1] : "";
};

const root = process.cwd();
const skipBuild = hasFlag("--skip-build");
const makeDmg = hasFlag("--dmg");
const makeZip = hasFlag("--zip");
const installApp = hasFlag("--install");
const dryRun = hasFlag("--dry-run");
const notaryProfile = flagVal("--notary-profile");
const signId = flagVal("--sign-id") || process.env.HEAV3NS_SIGN_ID || "Developer ID Application: BRIGHTMINDS LLC (C35F6CCNLU)";

const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
const cliVersion = pkg.version || "1.0.0";
const infoPlist = fs.readFileSync(path.join(root, "native/Info.plist"), "utf8");
const appVersion = infoPlist.match(/<key>CFBundleShortVersionString<\/key>\s*<string>([^<]+)<\/string>/)?.[1] || cliVersion;

const appDir = path.join(root, "build/HEAV3NS-CODE.app");
const distDir = path.join(root, "dist/releases");
const appExec = path.join(appDir, "Contents/MacOS/HEAV3NS-CODE");
const runtimeBin = path.join(appDir, "Contents/Resources/runtime/heav3ns-micro");
const nodeBin = path.join(appDir, "Contents/Resources/runtime/node");
const bundleRes = path.join(appDir, "Contents/Resources/HEAV3NS-CODE_Heav3nsCode.bundle");

function run(cmd, args, cwd = root) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  if (dryRun) return "";
  return execFileSync(cmd, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}

if (!skipBuild) {
  run("cargo", ["build", "--release"]);
  run("swift", ["build", "-c", "release"], path.join(root, "native"));
}

if (!dryRun) {
  if (!fs.existsSync(appDir)) throw new Error(`Missing app template bundle at ${appDir}`);
  fs.mkdirSync(path.dirname(runtimeBin), { recursive: true });
  fs.copyFileSync(path.join(root, "target/release/heav3ns-micro"), runtimeBin);
  fs.copyFileSync(path.join(root, "native/.build/release/HEAV3NS-CODE"), appExec);
  fs.rmSync(bundleRes, { recursive: true, force: true });
  fs.cpSync(path.join(root, "native/.build/release/HEAV3NS-CODE_Heav3nsCode.bundle"), bundleRes, { recursive: true });

  // Ensure @executable_path/../Frameworks is present in LC_RPATH so Sparkle.framework resolves reliably.
  const rpaths = execFileSync("otool", ["-l", appExec], { encoding: "utf8" });
  if (!rpaths.includes("@executable_path/../Frameworks")) {
    run("install_name_tool", ["-add_rpath", "@executable_path/../Frameworks", appExec]);
  }

  // Inside-out hardened runtime code signing
  if (fs.existsSync(runtimeBin)) {
    run("codesign", ["--force", "--sign", signId, "--options", "runtime", "--timestamp", runtimeBin]);
  }
  if (fs.existsSync(nodeBin)) {
    run("codesign", ["--force", "--sign", signId, "--options", "runtime", "--timestamp", "--entitlements", path.join(root, "native/Node.entitlements"), nodeBin]);
  }
  run("codesign", ["--force", "--deep", "--sign", signId, "--options", "runtime", "--timestamp", "--entitlements", path.join(root, "native/App.entitlements"), appDir]);
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=1", appDir]);
}

const artifacts = [];
if ((makeDmg || makeZip) && !dryRun) {
  fs.mkdirSync(distDir, { recursive: true });
  const dmgName = `HEAV3NS-CODE-${appVersion}-arm64.dmg`;
  const zipName = `HEAV3NS-CODE-${appVersion}-arm64.zip`;
  const cliTarName = `heav3ns-micro-${cliVersion}-darwin-arm64.tar.gz`;

  if (makeZip) {
    const zipPath = path.join(distDir, zipName);
    fs.rmSync(zipPath, { force: true });
    run("ditto", ["-c", "-k", "--keepParent", appDir, zipPath]);
    artifacts.push(zipPath);

    const cliTarPath = path.join(distDir, cliTarName);
    fs.rmSync(cliTarPath, { force: true });
    run("tar", ["-czf", cliTarPath, "-C", path.join(root, "target/release"), "heav3ns-micro"]);
    artifacts.push(cliTarPath);
  }

  if (makeDmg) {
    const stage = fs.mkdtempSync(path.join(os.tmpdir(), "heav3ns-dmg-"));
    try {
      run("ditto", [appDir, path.join(stage, "HEAV3NS-CODE.app")]);
      fs.symlinkSync("/Applications", path.join(stage, "Applications"));
      const dmgPath = path.join(distDir, dmgName);
      fs.rmSync(dmgPath, { force: true });
      run("hdiutil", ["create", "-volname", `HEAV3NS-CODE ${appVersion}`, "-srcfolder", stage, "-ov", "-format", "UDZO", dmgPath]);
      run("codesign", ["--force", "--sign", signId, "--timestamp", dmgPath]);
      if (notaryProfile) {
        run("xcrun", ["notarytool", "submit", dmgPath, "--keychain-profile", notaryProfile, "--wait"]);
        run("xcrun", ["stapler", "staple", dmgPath]);
      }
      artifacts.push(dmgPath);
    } finally {
      fs.rmSync(stage, { recursive: true, force: true });
    }
  }

  if (artifacts.length > 0) {
    const sums = artifacts.map((a) => execFileSync("shasum", ["-a", "256", path.basename(a)], { cwd: distDir, encoding: "utf8" }).trim()).join("\n") + "\n";
    const sumPath = path.join(distDir, "SHA256SUMS.txt");
    fs.writeFileSync(sumPath, sums, "utf8");
    console.log(`Generated checksums at ${sumPath}:\n${sums}`);
  }
}

if (installApp && !dryRun) {
  try { execFileSync("pkill", ["-x", "HEAV3NS-CODE"]); } catch { /* not running */ }
  fs.rmSync("/Applications/HEAV3NS-CODE.app", { recursive: true, force: true });
  run("ditto", [appDir, "/Applications/HEAV3NS-CODE.app"]);
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=1", "/Applications/HEAV3NS-CODE.app"]);
}

console.log(`release-app${dryRun ? " (dry-run)" : ""} OK — cli=v${cliVersion}, app=v${appVersion}, artifacts=${artifacts.length}`);
