// The composer owns keyboard submission; native text editing stays in the textarea.
export function attachComposer(draft, form) {
  let composing = false;
  let compositionKeyHandled = false;
  let finishingComposition = false;

  draft.addEventListener("compositionstart", () => {
    composing = true;
    compositionKeyHandled = false;
    finishingComposition = false;
  });
  draft.addEventListener("compositionend", (event) => {
    composing = false;
    // WebKit can dispatch the committing Return after compositionend.
    finishingComposition = Boolean(event.data) && !compositionKeyHandled;
  });
  draft.addEventListener("keydown", (event) => {
    if (event.key !== "Enter") {
      finishingComposition = false;
      if (composing && event.key === "Escape") compositionKeyHandled = true;
      return;
    }
    if (composing || event.isComposing) {
      compositionKeyHandled = true;
      return; // Let the input method perform its conversion.
    }
    if (event.shiftKey) return;
    event.preventDefault();
    if (finishingComposition) {
      finishingComposition = false;
      return;
    }
    form.requestSubmit();
  });
  draft.addEventListener("keyup", (event) => {
    if (event.key === "Enter" && !composing) finishingComposition = false;
  });
  const reset = () => {
    composing = false;
    compositionKeyHandled = false;
    finishingComposition = false;
  };
  draft.addEventListener("blur", reset);
  draft.addEventListener("pointerdown", () => {
    finishingComposition = false;
  });
}