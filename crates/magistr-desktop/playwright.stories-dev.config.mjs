import config from "./playwright.config.mjs";

export default {
  ...config,
  // Explicit capture requests use the same verified stories as static review.
  testMatch:process.env.MAGISTR_GUI_ITERATION?["storybook.dev.spec.mjs","storybook.spec.mjs"]:"storybook.dev.spec.mjs",
  testIgnore:[],
  webServer:{
    command:"npx storybook dev --host 127.0.0.1 --port 6007 --ci --no-open",
    url:"http://127.0.0.1:6007/index.json",
    reuseExistingServer:false,
    timeout:15000,
  },
};
