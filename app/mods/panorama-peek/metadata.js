const labels = { claude: 'Claude Code', codex: 'Codex', gemini: 'Gemini', cursor: 'Cursor', amp: 'Amp' };
const statuses = { fail: 'Failed', run: 'Working', rev: 'Reviewing', wait: 'Waiting', blk: 'Blocked', done: 'Done', cxl: 'Cancelled', q: 'Queued', bg: 'Background', inf: 'Inferred', lost: 'Lost', stale: 'Stale' };
function minutes(seconds) {
  const n = Math.max(0, Math.floor(seconds / 60));
  return n < 1 ? '<1m' : n < 60 ? `${n}m` : `${Math.floor(n / 60)}h`;
}
export function metadata(value, harness, now) {
  if (!value) return '';
  const agent = value.agent, root = value.root, status = agent?.status ?? root.herdr_status;
  const at = agent?.ended ?? agent?.last_at;
  const statusText = [statuses[status] ?? status, Number.isFinite(at) ? `${minutes(now / 1000 - at)} ago` : null].filter(Boolean).join(' ');
  const end = Number.isFinite(agent?.ended) ? agent.ended : ['run', 'rev', 'wait', 'blk', 'bg', 'q'].includes(status) ? now / 1000 : null;
  const duration = Number.isFinite(agent?.started) && Number.isFinite(end) && end >= agent.started ? minutes(end - agent.started) : null;
  return [statusText, labels[harness] ?? harness, agent?.model, duration, Number.isSafeInteger(agent?.tool_calls) ? `${agent.tool_calls} tools` : null].filter(v => v !== null && v !== undefined && v !== '').join(' · ');
}
