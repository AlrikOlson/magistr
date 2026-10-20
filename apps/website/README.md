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

`npm test` builds the site and checks its rendered content, links, metadata, screenshots and deployment assets. `test:ui` serves that build on loopback and checks the four routes in headless WebKit at desktop and compact sizes, in light and dark appearances. It does not control the screen or verify native assistive technology.

For a visual change, capture into a new dated iteration folder, then open and review every saved image and record the result there:

```sh
npm --prefix apps/website run test:ui -- --evidence-dir docs/evidence/desktop-iterations/YYYY-MM-DD-website-change
```

The capture command refuses to overwrite an existing PNG set. `browser-checks.json` records source hashes and capture metadata after each image, preserving partial evidence if a check fails; its pending review markers require a separate review record. Do not treat a successful test or capture as visual approval.

After deployment, run the same interaction checks against the live origin:

```sh
npm --prefix apps/website run test:ui -- --base-url https://magistr.dev
```

Add `--evidence-dir` with a new iteration folder when deployed captures are needed. For a change confined to documentation search, add `--evidence-scope search` to capture its idle, result, empty and unavailable states while still running all interaction checks. For a follow-up review of the home walkthrough and capture reader, `--evidence-scope refinement` captures those areas, their focus states and 320 px reading details. `--evidence-scope branding` captures the shared navigation on every route plus 320 px home reflow. All scopes still run the complete interaction checks. Local source hashes identify the checkout used for the check; separately compare deployed assets with the reviewed build before claiming they match.

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
