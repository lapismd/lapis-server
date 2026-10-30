const REQUIREMENT_ID = /^[A-Z]+(?:-[A-Z0-9]+)+-\d{3}/;

/** Visible contents label. Design-core takes this id from the h2 text. */
export function requirementTocLabel(heading: string): string {
  const text = heading.trim();
  return text.match(REQUIREMENT_ID)?.[0] ?? text;
}
