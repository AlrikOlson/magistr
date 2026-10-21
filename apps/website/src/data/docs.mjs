/** The content collection owns page order and relationships; this defines section labels. */
export const docSections = [
  { id: "overview", title: "Overview" },
  { id: "getting-started", title: "Getting started" },
  { id: "concepts", title: "Concepts" },
  { id: "working", title: "Working with Magistr" },
  { id: "reference", title: "Reference" },
  { id: "help", title: "Help and maintenance" },
  { id: "contributing", title: "Contributing" },
];

export const docPath = (id) => id === "index" ? "/docs" : `/docs/${id}`;

export function groupDocs(docs) {
  const ids = new Set(docs.map(doc => doc.id));
  const sections = new Set(docSections.map(section => section.id));
  if (!ids.has("index")) throw new Error("Documentation needs a home page.");
  const positions = new Set();
  for (const doc of docs) {
    if (!sections.has(doc.data.section)) throw new Error(`Unknown section for ${doc.id}`);
    const position = `${doc.data.section}:${doc.data.order}`;
    if (positions.has(position)) throw new Error(`Duplicate documentation order: ${position}`);
    positions.add(position);
    if (new Set(doc.data.related).size !== doc.data.related.length) throw new Error(`Duplicate related page for ${doc.id}`);
    for (const id of doc.data.related) {
      if (!ids.has(id) || id === doc.id) throw new Error(`Invalid related page ${id} for ${doc.id}`);
    }
  }
  return docSections.map(section => ({
    ...section,
    pages: docs.filter(doc => doc.data.section === section.id).sort((a, b) => a.data.order - b.data.order),
  })).filter(section => section.pages.length);
}
