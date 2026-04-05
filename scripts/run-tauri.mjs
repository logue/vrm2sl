#!/usr/bin/env node
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");
const envPath = path.join(repoRoot, ".env");

const loadEnvFallback = (filePath) => {
  const envContent = readFileSync(filePath, "utf8");

  for (const rawLine of envContent.split(/\r?\n/u)) {
    const line = rawLine.trim();

    if (!line || line.startsWith("#")) {
      continue;
    }

    const separatorIndex = line.indexOf("=");
    if (separatorIndex <= 0) {
      continue;
    }

    const key = line.slice(0, separatorIndex).trim();
    let value = line.slice(separatorIndex + 1).trim();

    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }

    if (!(key in process.env)) {
      process.env[key] = value;
    }
  }
};

const loadRepoEnv = () => {
  try {
    if (typeof process.loadEnvFile === "function") {
      process.loadEnvFile(envPath);
      return;
    }
  } catch {
    // Fall back to a simple parser when Node cannot load the env file directly.
  }

  loadEnvFallback(envPath);
};

const main = () => {
  const tauriArgs = process.argv.slice(2);
  if (tauriArgs.length === 0) {
    console.error("Usage: node scripts/run-tauri.mjs <tauri args...>");
    process.exit(1);
  }

  loadRepoEnv();

  const tauriCommand = process.platform === "win32" ? "tauri.cmd" : "tauri";
  const child = spawn(tauriCommand, tauriArgs, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  child.on("close", (code) => {
    process.exit(code ?? 1);
  });

  child.on("error", (error) => {
    console.error(error);
    process.exit(1);
  });
};

main();
