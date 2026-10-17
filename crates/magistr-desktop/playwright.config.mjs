import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/browser",
  testIgnore: ["storybook.spec.mjs","storybook.dev.spec.mjs"],
  outputDir: "../../.magistr/desktop-browser-results",
  workers: 1,
  retries: 0,
  reporter: "list",
  timeout: 15000,
  use: {
    browserName: "webkit",
    headless: true,
    viewport: {width: 1280, height: 820},
    deviceScaleFactor: 1,
    reducedMotion: "reduce",
    trace: "retain-on-failure",
  },
});
