# Image Asset Generation

## MCP Server: `mcp-image`

The `mcp-image` MCP server (Gemini-backed) is available for generating visual assets. Use the `generate_image` tool to create icons, illustrations, banners, and other brand assets.

**Output directory:** `.magistr/generated-images/`

## Automatic Transparent PNG Extraction

A PostToolUse hook (`post-image-gen.sh`) fires after every `generate_image` call. It automatically:
1. Takes the generated image
2. Runs `scripts/extract-foreground.py` to remove the background
3. Saves a `-transparent.png` variant alongside the original

**Example:** generating `hero-icon` produces:
- `.magistr/generated-images/hero-icon.png` (original, opaque)
- `.magistr/generated-images/hero-icon-transparent.png` (background removed)

## When to Use Which Version

| Use case | Version | Why |
|----------|---------|-----|
| Icons on themed card backgrounds | **transparent** | Icon floats on the card's background color |
| Feature/section icons in grids | **transparent** | Adapts to any container background |
| Hero/banner backgrounds | **opaque** | Fills the entire area, background is part of the design |
| Section illustrations (full-width) | **opaque** | Background matches site theme, no gaps |
| Social media cards (OG image, Twitter) | **opaque** | Must fill the entire card frame |
| Favicons, app icons | **opaque** | Square format, background is intentional |
| Profile pictures (circular crop) | **opaque** | Background fills the circle |
| README banners | **opaque** | Wide format, dark bg with content |

**Rule of thumb:** If the image IS the background or fills a fixed frame, use opaque. If the image sits ON TOP of another background, use transparent.

## Workflow for Asset Tasks

1. Generate the image with `generate_image` using a detailed prompt describing style, colors, and composition
2. The hook automatically produces the transparent variant
3. Use `sips` (macOS) or similar to resize for target dimensions
4. Copy the appropriate version (opaque or transparent) to the final location (e.g., `apps/website/public/`, `extensions/vscode/`)
5. Update the component/page that references the asset

## Brand Consistency

- Dark theme: use dark backgrounds (#1a1b26 Tokyo Night, #282a36 Dracula) for opaque assets
- Icon style: clean, minimal, geometric — match existing Magistr visual language
- Color palette: follow the project's theme system (Catppuccin, Dracula, Tokyo Night)
