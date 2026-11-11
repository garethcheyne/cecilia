/** `base`, or `base 2`, `base 3`… so it doesn't repeat a name already taken. */
export function uniqueName(base: string, taken: Iterable<string>): string {
  const names = new Set([...taken].map((n) => n.trim().toLowerCase()))
  const root = base.trim()
  if (!names.has(root.toLowerCase())) return root
  let n = 2
  while (names.has(`${root} ${n}`.toLowerCase())) n++
  return `${root} ${n}`
}
