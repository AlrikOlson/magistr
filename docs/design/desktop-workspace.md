# Desktop workspace — interaction restart

15 October 2026. The owner rejected the previous layout pass and status-card composition. This is the current design contract; prior screenshot iterations remain historical evidence.

## Human outcome

Returning to a project, the owner wants to choose what should happen next and understand its dependencies. Success means seeing prerequisites and consequences together, inspecting the actual plan without losing that context, and returning to discussion with the draft intact. This is an expert desktop workspace, used across several projects and interrupted conversations. No user study has been performed; the priority is the owner's explicit feedback.

## Product objects and authority

- **Work piece:** an actual roadmap item, with its short name, full title, state, recorded requirements and checklist. Dependency ports expose prerequisites and work it enables. Readiness and the next ready item come from the Rust workflow projection; geometry never determines eligibility or priority.
- **Dependency connection:** an authored `deps` relationship. Direction means prerequisite to dependent. Selecting it highlights both ends; unavailable records remain explicit. Cycles do not invent an execution order.
- **Investigation thread:** a recorded branch with its origin and conclusion. Its steps retain recorded order and identity. Thread grouping is distinct from supporting/refuting/revision relationships.
- **Decision:** the purpose and outcome of an actual reasoning step, with full context, rationale and original reasoning in an inspector. No inferred confidence, generated evidence or invented causal edges.
- **Work run:** the current or retained execution, with its actual tasks, check attempts and produced items. Starting work continues to use the application command owner; selecting or arranging a piece does not start or reprioritize it.

Native writers remain in SessionRegistry and workflow resources. `read_workflow` and `read_roadmap_item` project those observations into the desktop. Camera, selection and arrangement belong to the desktop only. There is no second workflow store.

## Spatial interaction

Use a crisp two-dimensional canvas for relationship geometry and native DOM elements for selectable text and controls. The work surface is an instrument for comparing dependencies, not a status dashboard. Pan the background, zoom around the pointer, fit the entire plan, select a work piece and keep it visible while reading its inspector. Directional controls and zoom/fit buttons provide single-pointer alternatives to dragging; keyboard controls provide the same reach. Moving a piece arranges the view only. Auto arrange restores dependency-based placement.

Work pieces and investigation threads have distinct forms. Work pieces expose input/output ports and discrete authored checklist marks. Reasoning threads use aligned decision columns, explicit branch headings, an origin and a conclusion; recorded revisions and evidence links remain separately identified. Relations in the same column use vertical ports rather than looping beside the column. Large prerequisite sets use a counted bundle that opens all recorded dependencies. A selected relationship emphasizes its endpoints and the surrounding route, not unrelated decoration.

## Complete journeys

1. Enter a project, open its work surface, identify ready work, select it, inspect prerequisites and what it enables, then return to the same camera and selection after reading a linked item.
2. Follow a decision into its investigation, inspect the source reasoning, follow a revision or conclusion, and return to the original thread and focus.
3. Switch between a conversation and project work without submitting or losing the draft, changing the resident agent, or repeating admission.
4. Inspect stale data with an explicit freshness message; retry through the existing read owner. A late response cannot replace a newer project or selection.
5. At minimum window size, keep the surface and inspector reachable, preserve their independent reading places and expose a direct close-inspector action.

## Frame and supporting views

The project is the stable context. Conversation, Roadmap, Reasoning, Work and Feedback are destinations rather than repeated buttons in unrelated headers. Global project selection is separate from contextual actions. The project map uses the same camera and routing owner for readable project/conversation pieces, with undirected membership lines. Execution, request handling, settings, project admission and saved conversations use the same density, focus and disclosure language. Full content stays available and copyable. Compact reply outlines open on demand, and the canvas control footer remains outside the drawing area.

## Verification

Capture each visible iteration in `docs/evidence/desktop-iterations/2026-10-15-restart/` with exact source receipts, scenario, viewport and theme. Inspect the images before accepting them. Verify pan, zoom, fit, selected relationships, keyboard navigation, pointer cancellation, context retention, stale reads, large plans, long text and both themes in background WebKit. Native OS-input and VoiceOver remain unverified without an authorized foreground run.

Canvas is not an accessibility tree: [MDN canvas reference](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/canvas). Drag interactions need a single-pointer alternative: [W3C dragging movements](https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements). These inform the implementation; they do not establish product-wide conformance.