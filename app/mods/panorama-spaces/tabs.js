// Commands return null; reconcile against the live model after every await.
export function graphTab(space, panes) {
  const graph = new Set(panes.filter(pane => pane.mod === 'panorama-graph').map(pane => pane.tab));
  return space?.tabs.find(tab => graph.has(tab.id)) ?? null;
}
export function createSwarmOpener({ run, spaces, panes, sleep, pending = new Map() }) {
  return function open(spaceId) {
    run('space.select', { id: spaceId });
    if (pending.has(spaceId)) return pending.get(spaceId);
    const task = (async () => {
      let space = spaces().find(space => space.id === spaceId);
      if (!space) return;
      let tab = graphTab(space, panes());
      if (tab) { run('tab.select', { id: tab.id }); return; }
      const before = new Set(space.tabs.map(tab => tab.id));
      run('tab.new', { space: spaceId, name: 'Swarm', mod: 'panorama-graph', props: { space: spaceId } });
      // A bounded reconciliation handles async app commands and rapid clicks.
      for (let attempt = 0; attempt < 40; attempt++) {
        await sleep(25);
        space = spaces().find(space => space.id === spaceId);
        if (!space) return;
        tab = graphTab(space, panes());
        if (!tab) continue;
        if (!before.has(tab.id)) run('tab.move', { id: tab.id, index: 1 });
        // A later space selection wins over an asynchronous creation.
        if (space.active) run('tab.select', { id: tab.id });
        return;
      }
    })();
    pending.set(spaceId, task);
    task.finally(() => { if (pending.get(spaceId) === task) pending.delete(spaceId); });
    return task;
  };
}
export function contentHeight(rows, groups = 0) {
  return Math.max(90, 31 + rows.reduce((height, row) => height + (row.tree ? 28 : 30), 0) + Math.max(0, rows.length - 1) * 4 + groups * 26 + 14);
}
