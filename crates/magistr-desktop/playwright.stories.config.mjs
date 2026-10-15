import config from "./playwright.config.mjs";
export default {
  ...config,
  testMatch:"storybook.spec.mjs",
  testIgnore:[],
  timeout:30000,
  webServer:{
    command:"npm run build:storybook && npx vite preview --outDir ../../.magistr/desktop-storybook --host 127.0.0.1 --port 6007 --strictPort",
    url:"http://127.0.0.1:6007/index.json",
    reuseExistingServer:false,
    timeout:60000,
  },
};
