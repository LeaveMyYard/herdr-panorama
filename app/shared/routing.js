// All addresses originate in the verified live app/native join, never agent IDs.
export function rootTarget(item) {
  return item.terminal ? { name: item.isNative ? 'terminal.peek' : 'agent.focus', args: { terminal: item.terminal } } : null;
}
export function detailTarget(item, row) {
  return { mod: 'panorama-peek', kind: 'popup', width: 284, props: { root: item.root, agent: row.agent, terminal: item.terminal, terminalAction: item.isNative ? 'peek' : 'focus' } };
}
