export default {
  framework: "@storybook/html-vite",
  stories: ["../stories/**/*.stories.mjs"],
  staticDirs: [{ from: "../ui", to: "/desktop-assets" }],
  core: { disableTelemetry: true },
  async viteFinal(config) {
    return {...config, server: {...config.server, host: "127.0.0.1"}};
  },
};
