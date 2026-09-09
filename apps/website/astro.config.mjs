// @ts-check
import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";
import sitemap from "@astrojs/sitemap";
import expressiveCode from "astro-expressive-code";
import mdx from "@astrojs/mdx";
import pagefind from "astro-pagefind";

export default defineConfig({
  site: "https://magistr.dev",
  image: {
    remotePatterns: [
      { protocol: "https", hostname: "**.githubusercontent.com" },
      { protocol: "https", hostname: "**.gravatar.com" },
    ],
  },
  integrations: [
    expressiveCode({
      themes: ["tokyo-night"],
      styleOverrides: {
        borderRadius: "0.75rem",
        codeFontFamily: "'JetBrains Mono', ui-monospace, monospace",
      },
    }),
    mdx(),
    sitemap(),
    pagefind(),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
