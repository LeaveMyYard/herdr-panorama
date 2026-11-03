// Pure adapter. The host supplies the reviewed, per-call verified callPrivate seam.
// The CLI owns its five-second total deadline; this adapter never retries an open.
const unavailable = () => ({ available: false, questions: false });
function exact(value, keys) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}
export function parseAvailability(result) {
  if (result.code !== 0) return unavailable();
  try {
    const value = JSON.parse(result.stdout);
    if (!exact(value, ['available', 'questions']) || typeof value.available !== 'boolean' || typeof value.questions !== 'boolean' || (!value.available && value.questions)) return unavailable();
    return value;
  } catch { return unavailable(); }
}
export function parseOpened(result) {
  if (result.code !== 0) return false;
  try { const value = JSON.parse(result.stdout); return exact(value, ['opened']) && value.opened === true; }
  catch { return false; }
}
export function questionArgument(question) {
  if (!Number.isSafeInteger(question) || question < 1) throw new RangeError('app-inbox question');
  return String(question);
}
export class InboxNavigation {
  constructor({ callPrivate, appHome }) {
    this.callPrivate = callPrivate;
    this.appHome = appHome;
    this.capabilities = unavailable();
    this.error = null;
    this.pendingAvailability = null;
    this.pendingOpen = null;
  }
  get busy() { return this.pendingOpen !== null; }
  canOpen(question = null) {
    return this.capabilities.available && (question === null || this.capabilities.questions);
  }
  refresh() {
    if (this.pendingAvailability) return this.pendingAvailability;
    const task = (async () => {
      try {
        const help = await this.callPrivate(['app-inbox', '--help']);
        if (help.code !== 0 || help.stdout.trim() !== 'panorama app-inbox v1') {
          this.capabilities = unavailable(); this.error = help.stderr || null; return this.capabilities;
        }
        const result = await this.callPrivate(['app-inbox', '--available', '--app-home', this.appHome]);
        this.capabilities = parseAvailability(result);
        this.error = result.code !== 0 ? result.stderr || null : null;
      } catch (error) {
        this.capabilities = unavailable(); this.error = String(error.message ?? error);
      }
      return this.capabilities;
    })();
    this.pendingAvailability = task;
    task.finally(() => { if (this.pendingAvailability === task) this.pendingAvailability = null; });
    return task;
  }
  open(question = null) {
    // One user gesture owns one pending navigation even if another key is pressed.
    if (this.pendingOpen) return this.pendingOpen;
    if (question !== null) questionArgument(question); // Usage rejected before any host I/O.
    if (!this.canOpen(question)) return Promise.resolve({ opened: false });
    const args = ['app-inbox', '--open'];
    if (question !== null) args.push('--question', questionArgument(question));
    args.push('--app-home', this.appHome);
    const task = (async () => {
      try {
        const result = await this.callPrivate(args);
        const opened = parseOpened(result);
        this.error = opened ? null : result.stderr || null;
        // opened means the existing navigation RPC was accepted, never shown/closed.
        return { opened };
      } catch (error) {
        this.error = String(error.message ?? error); return { opened: false };
      }
    })();
    this.pendingOpen = task;
    task.finally(() => { if (this.pendingOpen === task) this.pendingOpen = null; });
    return task;
  }
}
