/// <reference types="node" />

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

describe("popover appearance", () => {
  it("defines the editor surface tokens for light and dark appearances", () => {
    const [light, dark] = styles.split(/@media\s*\(prefers-color-scheme:\s*dark\)/);

    expect(light).toBeDefined();
    expect(dark).toBeDefined();
    for (const token of [
      "--surface",
      "--surface-raised",
      "--surface-muted",
      "--border",
      "--text",
      "--muted",
      "--accent",
      "--warning",
      "--danger",
    ]) {
      expect(light).toContain(`${token}:`);
      expect(dark).toContain(`${token}:`);
    }
    expect(styles).toContain(".sheet {");
    expect(styles).toContain(".url-editor-row {");
    expect(styles).toContain(".window-editor-row {");
    expect(styles).toContain(".settings-card");
  });
});
