// Counts for inventory labels; canonical spaceTotals owns needs and urgency.
export function spaceInventory(roots, space) {
  const seen = new Set(), items = [];
  for (const root of roots) for (const item of [root, ...(root.nativeChildren ?? [])]) {
    const address = item.stock?.address ?? item.terminal;
    if (item.space !== space || !address || seen.has(address)) continue;
    seen.add(address); items.push(item);
  }
  return { orchestrators: items.filter(item => !item.isNative).length,
    threads: items.reduce((count, item) => count + item.root.threads.length, 0),
    agents: items.reduce((count, item) => count + Number(item.isNative) + item.root.threads.reduce((n, thread) => n + thread.agents.length, 0), 0) };
}
