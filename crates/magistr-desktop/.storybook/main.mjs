export default {
  framework: "@storybook/html-vite",
  stories: ["../stories/**/*.stories.mjs"],
  staticDirs: [{ from: "../ui", to: "/desktop-assets" }],
  core: { disableTelemetry: true },
  async viteFinal(config) {
    const {mergeConfig}=await import("vite");
    // Native build products share this crate root but never affect a story.
    return mergeConfig(config,{server:{host:"127.0.0.1",watch:{
      ignored:["**/target/**","**/gen/**","**/__pycache__/**"],
    }}});
  },
};
