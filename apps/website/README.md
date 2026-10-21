# Magistr website

This directory builds the static site at https://magistr.dev. Use the Node version in the repository-root `.node-version`; the package engine range is the framework's supported minimum.

From the repository root:

```sh
npm ci --prefix apps/website
npm --prefix apps/website test
npm --prefix apps/website exec -- playwright install webkit
npm --prefix apps/website run test:ui
npm --prefix apps/website audit
```

`npm test` builds the site and checks its rendered content, links, metadata, screenshots and deployment assets. `test:ui` serves that build on loopback and checks the nine routes in headless WebKit at desktop and compact sizes, in light and dark appearances. It does not control the screen or verify native assistive technology.

For a visual change, capture into a new dated iteration folder, then open and review every saved image and record the result there:

```sh
npm --prefix apps/website run test:ui -- --evidence-dir docs/evidence/desktop-iterations/YYYY-MM-DD-website-change
```

The capture command refuses to overwrite an existing PNG set. `browser-checks.json` records source hashes and capture metadata after each image, preserving partial evidence if a check fails; its pending review markers require a separate review record. Do not treat a successful test or capture as visual approval.

After deployment, run the same interaction checks against the live origin:

```sh
npm --prefix apps/website run test:ui -- --base-url https://magistr.dev
```

Add `--evidence-dir` with a new iteration folder when deployed captures are needed. For a change confined to documentation search, add `--evidence-scope search` to capture its idle, result, empty and unavailable states while still running all interaction checks. `--evidence-scope recovery` includes those search states and the 404 page in all four profiles. For a follow-up review of the home walkthrough and capture reader, `--evidence-scope refinement` captures those areas, their focus states and 320 px reading details. `--evidence-scope branding` captures the shared navigation on every route plus 320 px home reflow. For a complete copy revision, `--evidence-scope copy` captures full pages in all four profiles and readable section frames at desktop/light and compact/dark, plus the new guide at 320 px. `--evidence-scope session-replay` captures the real Codex conversation, six build moments and the optional saved plan, source and failed-check inspection, keyboard scrubbing, reflow and unavailable/no-script states. All scopes still run the complete interaction checks. Local source hashes identify the checkout used for the check; separately compare deployed assets with the reviewed build before claiming they match.

## Exploring Magistr

The landing page's four feature views explain roadmaps, saved reasoning, work chunks and checks. `ProjectTour.astro` renders the views; `src/data/product-tour.mjs` owns their plain-language explanations and selected relationships. `src/scripts/product-tour.ts` handles record selection and links into the recorded build. These are website explanations, not screenshots of a finished desktop planning interface.

`public/examples/project-records.json` is a dated excerpt from Magistr's actual roadmap and this website's saved design decision. The native roadmap and reasoning engines own the live records. Do not invent statuses, dependencies or historical reasoning when changing the presentation. Update the snapshot only from a new actual capture, with its date and scope stated. The inbox tasks and test results come from the separate original recording below; the page names the source of each example.

The seven guides cover installation, an initial project, roadmap planning, reasoning and research, chunked work, verification, and saved work across sessions. Their source is `src/content/docs/`; the shared navigation lives in `src/layouts/Docs.astro`.

Use `--evidence-scope revamp` for all nine page frames, the feature explorer, new guides and meaningful selected/empty/recovery states. `--evidence-scope revamp-finish` is restricted to the opening, roadmap caption, continued-work section and their reflow states; it is only sufficient alongside the full preceding review when those are the only areas changed. All interaction checks still run.

## Recorded project example

The landing page replays one real Codex development session. `public/demo/inbox/recording.json` contains 47 captured Magistr tool actions and four recorded agent messages and two paraphrased user requests from the matching local Codex history. `session-replay.mjs` merges them in time order. A single cursor drives the terminal transcript, saved plan, decision, check results and source snapshots; scrubbing backward must never expose a future file or completed task. `Workflow.astro` owns the markup and `project-recording.ts` owns playback. The feature tour uses the same session clock.

The initial view is the Codex session and a compact transport. Saved tasks, decisions, files and full output are closed until requested. The narrative sidebar and separate chapter strip are omitted. The complete static transcript appears only when playback is unavailable. Playback is opt-in, follows recorded time gaps at 1×, 4× or 8×, and stops on manual navigation, inspection, focus changes or leaving the page. A native range input and previous/next controls reach every entry. The full transcript and downloads remain available without JavaScript or when the recording fetch fails. The browser reconstructs a CLI session; it is not a terminal screen recording.

`scripts/publish-recording.mjs` owns the publication allowlist and text scrubbing. It retains only the six individually reviewed conversation entries and fields needed for the actual implementation. User requests are rewritten with the owner's permission and use ordinary speaker labels. A short provenance note explains that the session is adapted. Never publish a raw rollout. System/developer instructions, private model reasoning, unrelated messages, session identifiers and machine metadata are excluded. Personal filesystem prefixes in output become `[project]` or `[home]`; the disclosed saved project decision is a real Magistr record. Review all retained text and source before exporting, scan for secrets and identifiers, and regenerate the recording hash in the manifest. The exporter does not run against a user's history during a normal build.

The source bytes, original timestamps and pass/fail outcomes remain unchanged. Tests compare reconstructed files with `examples/support-inbox` and their captured write hashes, verify chronology and state at earlier positions, and exercise publication filtering. The downloadable source, app video and screenshots retain their original hashes in `manifest.json`. Before/after app captures remain in `public/demo/inbox/walkthrough/`, with their capture provenance. New app changes require a new recording; never rewrite a past run to imply it built different code.

Use `--evidence-scope session-replay` for this section. Capture the app itself with `node examples/support-inbox/record-browser.mjs docs/evidence/desktop-iterations/YYYY-MM-DD-inbox-change` only when the app changes.

## Deployment

The existing Cloudflare Pages project is **`magistr`**, with production branch **`main`**, serving **`magistr.pages.dev`** and **`magistr.dev`**. Its output is **`apps/website/dist`**. Reuse that project. There is no website worker or runtime secret to configure.

Deploy only after the reviewed source is committed and the static build and focused checks pass. With the repository's pinned Wrangler installed and an authorized Cloudflare login:

```sh
npm ci --prefix scripts
./scripts/node_modules/.bin/wrangler pages deploy apps/website/dist \
  --project-name magistr --branch main \
  --commit-hash "$(git rev-parse HEAD)" --commit-dirty=false
./scripts/public-link-check.sh --stage website --site https://magistr.dev
```

The deployment command publishes the site. It is separate from building or testing locally. Record the deployment ID, source commit and actual post-deployment content-check receipt. Compare the immutable deployment's files with `dist`, then compare the custom domain's HTML and the exact versioned image/icon URLs referenced by that HTML and the web manifest. Older unversioned image URLs can remain in CDN caches; checking only a cache-busted URL that the page never uses is not sufficient. Never commit access tokens or account credentials.

## Release content

The home page and installation guide intentionally describe the source preview while registry packages are unavailable. Version display reads the product version from the generated npm package manifest; do not maintain another version here. After actual npm/Cargo publication, replace preview-only installation and upgrade instructions with the verified registry commands, run the same website checks and visual review, then run:

```sh
./scripts/public-link-check.sh --stage release --site https://magistr.dev
```

The `website` stage checks reachable, current setup/recovery instructions. The `publication` stage also checks public repository/support routes. The `release` stage additionally requires the expected npm and Cargo versions and published installation copy. None of these stages publishes a package.