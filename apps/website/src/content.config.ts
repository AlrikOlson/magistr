import { defineCollection } from "astro:content";
import { z } from "astro/zod";
import { glob } from "astro/loaders";
import { docSections } from "./data/docs.mjs";

const docs = defineCollection({
  loader: glob({ pattern: "**/*.mdx", base: "./src/content/docs" }),
  schema: z.object({
    title: z.string().min(1),
    description: z.string().min(1),
    section: z.enum(docSections.map(section => section.id)),
    order: z.number().int().nonnegative(),
    related: z.array(z.string()).min(1),
  }),
});
export const collections = { docs };
