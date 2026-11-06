// Counts for inventory labels; canonical spaceTotals owns needs and urgency.
export function spaceInventory(roots, space) {
  const seen = new Set(), items = [];
  for (const root of roots) for (const item of [root, ...(root.nativeChildren ?? [])]) {
    const address = item.stock?.address ?? item.terminal;
    if (item.space !== space || !address || seen.has(address)) continue;
    seen.add(address); items.push(item);
  }
  return { roots: items.filter(item => !item.isNative).length,
    agents: items.reduce((count, item) => count + Number(item.isNative) + (item.root.agents?.length ?? 0), 0) };
}
