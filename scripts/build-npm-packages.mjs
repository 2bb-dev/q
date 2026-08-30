#!/usr/bin/env node
// Assembles the publishable npm packages from prebuilt release artifacts.
//
// Usage: node scripts/build-npm-packages.mjs <version> <artifacts-dir> <out-dir>
//
// <artifacts-dir> must contain the release archives produced by CI:
//   q-macos-aarch64.tar.gz, q-macos-x86_64.tar.gz,
//   q-linux-x86_64.tar.gz, q-windows-x86_64.zip
//
// <out-dir> receives one directory per package: the platform packages
// (q-cli-darwin-arm64, ...) and the main q-cli package.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [version, artifactsDir, outDir] = process.argv.slice(2);
if (!version || !artifactsDir || !outDir) {
  console.error(
    "usage: build-npm-packages.mjs <version> <artifacts-dir> <out-dir>",
  );
  process.exit(1);
}

const repoRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

const PLATFORMS = [
  {
    name: "q-cli-darwin-arm64",
    archive: "q-macos-aarch64.tar.gz",
    os: "darwin",
    cpu: "arm64",
    binary: "q",
  },
  {
    name: "q-cli-darwin-x64",
    archive: "q-macos-x86_64.tar.gz",
    os: "darwin",
    cpu: "x64",
    binary: "q",
  },
  {
    name: "q-cli-linux-x64",
    archive: "q-linux-x86_64.tar.gz",
    os: "linux",
    cpu: "x64",
    binary: "q",
  },
  {
    name: "q-cli-win32-x64",
    archive: "q-windows-x86_64.zip",
    os: "win32",
    cpu: "x64",
    binary: "q.exe",
  },
];

fs.rmSync(outDir, { recursive: true, force: true });

for (const platform of PLATFORMS) {
  const archive = path.join(artifactsDir, platform.archive);
  if (!fs.existsSync(archive)) {
    console.error(`missing artifact: ${archive}`);
    process.exit(1);
  }
  const binDir = path.join(outDir, platform.name, "bin");
  fs.mkdirSync(binDir, { recursive: true });
  if (platform.archive.endsWith(".zip")) {
    execFileSync("unzip", ["-o", archive, platform.binary, "-d", binDir]);
  } else {
    execFileSync("tar", ["xzf", archive, "-C", binDir, platform.binary]);
  }
  const manifest = {
    name: platform.name,
    version,
    description: `${platform.os}-${platform.cpu} binary for q-cli`,
    license: "MIT",
    repository: { type: "git", url: "git+https://github.com/2bb-dev/q.git" },
    os: [platform.os],
    cpu: [platform.cpu],
    files: [`bin/${platform.binary}`],
  };
  fs.writeFileSync(
    path.join(outDir, platform.name, "package.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

// Main package: copy the checked-in template and stamp versions.
const mainDir = path.join(outDir, "q-cli");
fs.cpSync(path.join(repoRoot, "npm", "q-cli"), mainDir, { recursive: true });
fs.copyFileSync(
  path.join(repoRoot, "README.md"),
  path.join(mainDir, "README.md"),
);
const manifestPath = path.join(mainDir, "package.json");
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
manifest.version = version;
for (const name of Object.keys(manifest.optionalDependencies)) {
  manifest.optionalDependencies[name] = version;
}
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

console.log(`built npm packages for ${version} in ${outDir}`);
