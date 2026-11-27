#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * Mac App Store (MAS) Compliant Packager & Signer for HEAV3NS-CODE.app + heav3ns-micro.
 *
 * Strips Sparkle.framework and legacy Node runtime, enables App Sandbox entitlements,
 * injects LSApplicationCategoryType & ITSAppUsesNonExemptEncryption, signs with
 * Apple Distribution + 3rd Party Mac Developer Installer, and optionally validates/uploads.
 *
 * Usage:
 *   node bin/release-mas.mjs [--skip-build] [--profile <path>] [--bundle-id <id>]
 *                            [--app-sign-id <id>] [--installer-sign-id <id>]
 *                            [--validate] [--upload] [--api-key <id>] [--api-issuer <uuid>]
 */
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execFileSync } from "node:child_process";

const argv = process.argv.slice(2);
const hasFlag = (f) => argv.includes(f);
const flagVal = (f) => {
  const i = argv.indexOf(f);
  return i >= 0 && i + 1 < argv.length && !argv[i + 1].startsWith("--") ? argv[i + 1] : "";
};

const root = process.cwd();
const skipBuild = hasFlag("--skip-build");
const validatePkg = hasFlag("--validate");
const uploadPkg = hasFlag("--upload");
const appSignId = flagVal("--app-sign-id") || process.env.HEAV3NS_MAS_APP_SIGN_ID || "Apple Distribution: BRIGHTMINDS LLC (C35F6CCNLU)";
const installerSignId = flagVal("--installer-sign-id") || process.env.HEAV3NS_MAS_INSTALLER_SIGN_ID || "3rd Party Mac Developer Installer: BRIGHTMINDS LLC (C35F6CCNLU)";
const apiKey = flagVal("--api-key") || process.env.ASC_API_KEY || "";
const apiIssuer = flagVal("--api-issuer") || process.env.ASC_API_ISSUER || "";

const basePlist = fs.readFileSync(path.join(root, "native/Info.plist"), "utf8");
const defaultBundleId = basePlist.match(/<key>CFBundleIdentifier<\/key>\s*<string>([^<]+)<\/string>/)?.[1] || "io.heav3ns.code";
const bundleId = flagVal("--bundle-id") || defaultBundleId;
const appVersion = basePlist.match(/<key>CFBundleShortVersionString<\/key>\s*<string>([^<]+)<\/string>/)?.[1] || "16.4.1";

function run(cmd, args, cwd = root, env = process.env) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  return execFileSync(cmd, args, { cwd, env, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}

function findMatchingProfile(targetBundleId) {
  const explicit = flagVal("--profile");
  if (explicit) return explicit;
  const dirs = [
    path.join(os.homedir(), "Library/Developer/Xcode/UserData/Provisioning Profiles"),
    path.join(os.homedir(), "Library/MobileDevice/Provisioning Profiles")
  ];
  for (const dir of dirs) {
    if (!fs.existsSync(dir)) continue;
    for (const entry of fs.readdirSync(dir)) {
      if (!entry.endsWith(".provisionprofile")) continue;
      const full = path.join(dir, entry);
      try {
        const decoded = execFileSync("security", ["cms", "-D", "-i", full], { encoding: "utf8" });
        if (decoded.includes(`.${targetBundleId}</string>`)) return full;
      } catch { /* ignore unreadable profile */ }
    }
  }
  return "";
}

function writeMasInfoPlist(srcPlistPath, destPlistPath, targetBundleId) {
  fs.copyFileSync(srcPlistPath, destPlistPath);
  const sparkleKeys = [
    "SUFeedURL", "SUPublicEDKey", "SUScheduledCheckInterval", "SURequireSignedFeed",
    "SUVerifyUpdateBeforeExtraction", "SUSignedFeedFailureExpirationInterval",
    "SUAutomaticallyUpdate", "SUEnableSystemProfiling"
  ];
  for (const key of sparkleKeys) {
    try { execFileSync("plutil", ["-remove", key, destPlistPath], { stdio: "ignore" }); } catch { /* key absent */ }
  }
  run("plutil", ["-replace", "CFBundleIdentifier", "-string", targetBundleId, destPlistPath]);
  run("plutil", ["-replace", "LSApplicationCategoryType", "-string", "public.app-category.developer-tools", destPlistPath]);
  run("plutil", ["-replace", "ITSAppUsesNonExemptEncryption", "-bool", "NO", destPlistPath]);
  run("plutil", ["-lint", destPlistPath]);
}

if (!skipBuild) {
  run("cargo", ["build", "--release"]);
  run("swift", ["build", "-c", "release"], path.join(root, "native"), { ...process.env, HEAV3NS_MAS: "1" });
}

const templateApp = path.join(root, "build/HEAV3NS-CODE.app");
const masApp = path.join(root, "build/HEAV3NS-CODE-MAS.app");
const distDir = path.join(root, "dist/releases");
const masPkg = path.join(distDir, `HEAV3NS-CODE-${appVersion}-mas.pkg`);

if (!fs.existsSync(templateApp)) throw new Error(`Missing app template bundle at ${templateApp}`);
fs.rmSync(masApp, { recursive: true, force: true });
fs.mkdirSync(path.join(masApp, "Contents/MacOS"), { recursive: true });
fs.mkdirSync(path.join(masApp, "Contents/Resources/runtime"), { recursive: true });
fs.mkdirSync(distDir, { recursive: true });

fs.copyFileSync(path.join(root, "native/.build/release/HEAV3NS-CODE"), path.join(masApp, "Contents/MacOS/HEAV3NS-CODE"));
fs.copyFileSync(path.join(root, "target/release/heav3ns-micro"), path.join(masApp, "Contents/Resources/runtime/heav3ns-micro"));
fs.chmodSync(path.join(masApp, "Contents/MacOS/HEAV3NS-CODE"), 0o755);
fs.chmodSync(path.join(masApp, "Contents/Resources/runtime/heav3ns-micro"), 0o755);

fs.cpSync(
  path.join(root, "native/.build/release/HEAV3NS-CODE_Heav3nsCode.bundle"),
  path.join(masApp, "Contents/Resources/HEAV3NS-CODE_Heav3nsCode.bundle"),
  { recursive: true }
);

const staticResources = [
  "AppIcon.icns", "LICENSE", "LICENSE-MAP.md", "NOTICE",
  "PATENT-NON-AGGRESSION-PLEDGE.md", "THIRD-PARTY-NOTICES.md", "TRADEMARKS.md"
];
for (const file of staticResources) {
  const src = path.join(templateApp, "Contents/Resources", file);
  if (fs.existsSync(src)) fs.copyFileSync(src, path.join(masApp, "Contents/Resources", file));
}

writeMasInfoPlist(path.join(root, "native/Info.plist"), path.join(masApp, "Contents/Info.plist"), bundleId);
fs.writeFileSync(path.join(masApp, "Contents/PkgInfo"), "APPL????", "utf8");

const profilePath = findMatchingProfile(bundleId);
if (profilePath) {
  fs.copyFileSync(profilePath, path.join(masApp, "Contents/embedded.provisionprofile"));
  console.log(`Embedded provisioning profile: ${profilePath}`);
} else {
  console.warn(`WARN: No Mac App Store .provisionprofile found for ${bundleId}. App Store Connect upload requires embedded.provisionprofile.`);
}

run("codesign", [
  "--force", "--sign", appSignId, "--options", "runtime", "--timestamp",
  "--entitlements", path.join(root, "native/Helper.mas.entitlements"),
  path.join(masApp, "Contents/Resources/runtime/heav3ns-micro")
]);
run("codesign", [
  "--force", "--sign", appSignId, "--options", "runtime", "--timestamp",
  "--entitlements", path.join(root, "native/App.mas.entitlements"),
  masApp
]);
run("codesign", ["--verify", "--deep", "--strict", "--verbose=1", masApp]);

fs.rmSync(masPkg, { force: true });
run("productbuild", ["--component", masApp, "/Applications", "--sign", installerSignId, masPkg]);
run("pkgutil", ["--check-signature", masPkg]);

if ((validatePkg || uploadPkg) && (!apiKey || !apiIssuer)) {
  throw new Error("App Store Connect validation/upload requires --api-key <keyId> and --api-issuer <issuerUuid> (or ASC_API_KEY / ASC_API_ISSUER).");
}
if (validatePkg) {
  run("xcrun", ["altool", "--validate-app", "-f", masPkg, "-t", "macos", "--apiKey", apiKey, "--apiIssuer", apiIssuer]);
}
if (uploadPkg) {
  run("xcrun", ["altool", "--upload-app", "-f", masPkg, "-t", "macos", "--apiKey", apiKey, "--apiIssuer", apiIssuer]);
}

console.log(`release-mas OK — bundleId=${bundleId}, version=${appVersion}, pkg=${masPkg}`);
