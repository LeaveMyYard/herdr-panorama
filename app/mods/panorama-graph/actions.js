import { rootTarget } from './routing.js';
// Read-only navigation intent. A question ID never becomes text sent to a pane.
export function answerIntent(item, node) {
  const subject = node?.item ?? item;
  if (!subject) return null;
  const agent = node?.agent;
  if (agent ? agent.status === 'blk' : subject.status === 'blocked') {
    const target = rootTarget(subject);
    return target ? { kind: 'permission', target } : null;
  }
  const questions = agent ? agent.question_ids : subject.root.inbox;
  const question = questions.filter(value => Number.isSafeInteger(value) && value > 0).sort((a, b) => a - b)[0];
  return question === undefined ? null : { kind: 'question', question };
}
export function transcriptProps(item, node, thread) {
  const subject = node?.item ?? item;
  if (!subject) return null;
  return { root: subject.root, thread: node?.thread ?? thread ?? null, agent: node?.agent ?? null, terminal: subject.terminal, terminalAction: subject.isNative ? 'peek' : 'focus', action: 'transcript' };
}
