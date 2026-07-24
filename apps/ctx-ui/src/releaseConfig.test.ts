/// <reference types="node" />

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface TauriConfig {
  bundle: {
    targets: string[];
    createUpdaterArtifacts: boolean;
  };
  plugins: {
    updater: {
      pubkey: string;
      endpoints: string[];
    };
  };
}

const config = JSON.parse(
  readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
) as TauriConfig;
const workflow = readFileSync(
  resolve(process.cwd(), "../../.github/workflows/release.yml"),
  "utf8",
);

describe("signed release configuration", () => {
  it("builds an updater-enabled macOS app target alongside the DMG", () => {
    expect(config.bundle.targets).toEqual(["app", "dmg"]);
    expect(config.bundle.createUpdaterArtifacts).toBe(true);
  });

  it("publishes a GitHub manifest using Actions signing secrets", () => {
    expect(config.plugins.updater.pubkey).not.toHaveLength(0);
    expect(config.plugins.updater.endpoints).toEqual([
      "https://github.com/Jaygaur99/ctx/releases/latest/download/latest.json",
    ]);
    expect(workflow).toContain("uploadUpdaterJson: true");
    expect(workflow).toContain("secrets.TAURI_SIGNING_PRIVATE_KEY");
    expect(workflow).toContain("secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD");
  });
});
