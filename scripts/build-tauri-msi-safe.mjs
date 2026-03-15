#!/usr/bin/env node
import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const configPath = path.join(repoRoot, "backend", "tauri.conf.json");
const tempConfigPath = path.join(
  repoRoot,
  "backend",
  "tauri.conf.msi-temp.json",
);

function toMsiCompatibleVersion(version) {
  // MSI requires MAJOR.MINOR.PATCH only — strip pre-release and build metadata.
  const match = version.match(/^(\d+\.\d+\.\d+)/);
  return match ? match[1] : version;
}

function runTauriBuild(configToUse, extraArgs) {
  const tauriCommand = process.platform === "win32" ? "tauri.cmd" : "tauri";
  return new Promise((resolve) => {
    const child = spawn(
      tauriCommand,
      ["build", "--config", configToUse, ...extraArgs],
      {
        cwd: repoRoot,
        stdio: "inherit",
        shell: process.platform === "win32",
      },
    );

    child.on("close", (code) => resolve(code ?? 1));
    child.on("error", () => resolve(1));
  });
}

async function main() {
  const rawArgs = process.argv.slice(2);
  const extraArgs = rawArgs[0] === "--" ? rawArgs.slice(1) : rawArgs;

  if (process.platform !== "win32") {
    const code = await runTauriBuild(configPath, extraArgs);
    process.exit(code);
  }

  const rawConfig = await fs.readFile(configPath, "utf8");
  const tauriConfig = JSON.parse(rawConfig);

  const originalVersion = String(tauriConfig.version ?? "");
  const msiVersion = toMsiCompatibleVersion(originalVersion);
  tauriConfig.version = msiVersion;

  // Windows FS is case-insensitive: if mainBinaryName is a case variant of
  // productName (e.g. "Vrm2SL" vs "vrm2sl"), the GUI binary and the CLI
  // binary ("vrm2sl.exe") land in the same slot → WiX ICE30.
  // Append "-tauri" so the MSI installs "Vrm2SL-tauri.exe" (unambiguous).
  // The original tauri.conf.json is never touched.
  const mainBin = String(tauriConfig.mainBinaryName ?? "").toLowerCase();
  const prodName = String(tauriConfig.productName ?? "").toLowerCase();
  if (mainBin && mainBin === prodName) {
    tauriConfig.mainBinaryName = `${tauriConfig.mainBinaryName}-tauri`;
  }

  await fs.writeFile(
    tempConfigPath,
    `${JSON.stringify(tauriConfig, null, 2)}\n`,
    "utf8",
  );

  try {
    if (originalVersion !== msiVersion) {
      console.log(
        `Windows MSI workaround: ${originalVersion} -> ${msiVersion}`,
      );
    }
    if (mainBin && mainBin === prodName) {
      console.log(
        `Windows MSI workaround: mainBinaryName ${mainBin} -> ${tauriConfig.mainBinaryName}`,
      );
    }

    const code = await runTauriBuild(tempConfigPath, extraArgs);
    process.exit(code);
  } finally {
    await fs.rm(tempConfigPath, { force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
