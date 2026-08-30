#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");

const PLATFORM_PACKAGES = {
  "darwin arm64": "q-cli-darwin-arm64",
  "darwin x64": "q-cli-darwin-x64",
  "linux x64": "q-cli-linux-x64",
  "win32 x64": "q-cli-win32-x64",
};

const key = `${process.platform} ${process.arch}`;
const pkg = PLATFORM_PACKAGES[key];
if (!pkg) {
  console.error(`q: unsupported platform: ${key}`);
  console.error("Prebuilt binaries: https://github.com/2bb-dev/q/releases");
  process.exit(1);
}

const binaryName = process.platform === "win32" ? "q.exe" : "q";
let binary;
try {
  binary = require.resolve(`${pkg}/bin/${binaryName}`);
} catch {
  console.error(`q: platform package ${pkg} is not installed.`);
  console.error(
    "Reinstall q-cli without --no-optional so npm can fetch the binary.",
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`q: failed to run ${binary}: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
