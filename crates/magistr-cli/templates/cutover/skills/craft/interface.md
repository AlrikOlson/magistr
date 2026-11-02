# Interface evidence

Read this before changing anything a person sees.

- Read the interface's architecture and design contract. Test with its real
  renderer and interaction tests; run only the affected scenarios.
- Use background renderer checks for layout and document behavior. Native
  input checks that take over the foreground, keyboard or clipboard need the
  person's explicit permission. Headless checks do not prove native input or
  assistive-technology behavior.
- Verify meaningful states, keyboard use, visual hierarchy and every supported
  light and dark mode.
- Capture updated screenshots of each changed area before committing, straight
  into the project's dated evidence folder, and open them from there. Keep view
  names, viewport, appearance, renderer, capture time, source hashes and review
  notes consistent. Commit the evidence with the change and keep earlier
  captures; a final-only gallery is not enough.
- Visual review is its own required check. For each image, record the person's
  task, what you observed and a verdict; judge layout and relationships as well
  as text and spacing. Keep the `visual review` check failed while defects
  remain. Fix, capture again and review again before finishing the step.
