// @ts-check
import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";
import sitemap from "@astrojs/sitemap";
import mdx from "@astrojs/mdx";
import pagefind from "astro-pagefind";
import {codeThemes} from "./src/data/code-theme.mjs";

export default defineConfig({
  site: "https://magistr.dev",
  integrations: [mdx(), sitemap(), pagefind()],
  markdown: {
    shikiConfig: { themes: codeThemes },
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
