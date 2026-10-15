// Decorative identity derives from the canonical project ID, never its current name or path.
export function decorateProject(element, project) {
  let hash = 2166136261;
  for (const character of String(project.id)) hash = Math.imul(hash ^ character.codePointAt(0), 16777619);
  element.dataset.projectMotif = String((hash >>> 16) % 4);
  element.style.setProperty("--project-angle", ((hash >>> 8) % 140 - 70) + "deg");
  return Array.from(project.name.trim())[0]?.toLocaleUpperCase() ?? "·";
}