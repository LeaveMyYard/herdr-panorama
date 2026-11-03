// Bounded line pages through the existing native hc adapter; no shell or fallback.
export const PAGE_LINES = 100;
const record = v => v !== null && typeof v === 'object' && !Array.isArray(v);
const integer = v => Number.isSafeInteger(v) && v >= 0;
export function parsePage(result, offset) {
  const raw = result?.stderr || result?.stdout || JSON.stringify(result);
  if (result?.code !== 0) return { error: raw };
  let value;
  try { value = JSON.parse(result.stdout); } catch (error) { return { error: String(error) }; }
  if (!record(value) || typeof value.text !== 'string' || !['first', 'last', 'lines'].every(k => integer(value[k]))
    || value.first !== offset || value.last > value.lines) return { error: raw };
  const count = value.text ? (value.text.match(/\n/g)?.length ?? 0) + (value.text.endsWith('\n') ? 0 : 1) : 0;
  if (count === 0 ? !(value.text === '' && value.first === 1 && value.last === 0 && value.lines === 0)
    : value.last < value.first || value.last - value.first + 1 !== count) return { error: raw };
  return { page: { text: value.text, first: value.first, last: value.last, lines: value.lines } };
}
export class Pages {
  constructor(read, current) {
    this.read = read; this.current = current; this.page = null; this.error = null;
    this.history = []; this.intent = 0; this.closed = false; this.pending = false;
  }
  async load(scope, offset = 1, history = []) {
    if (this.closed || !this.current(scope)) return false;
    const intent = ++this.intent;
    if (this.page && this.page.scope !== scope.key) { this.page = null; this.history = []; this.error = null; }
    this.pending = true;
    let answer;
    try {
      answer = parsePage(await this.read(['file', 'read', '--json', '--offset', String(offset), '--limit', String(PAGE_LINES), scope.path]), offset);
    } catch (error) { answer = { error: String(error) }; }
    if (this.closed || intent !== this.intent) return false;
    this.pending = false;
    if (!this.current(scope)) return false;
    if (answer.error !== undefined) { this.error = { raw: answer.error, scope: scope.key, path: scope.path, offset }; return false; }
    this.page = { ...answer.page, scope: scope.key, path: scope.path };
    this.history = history; this.error = null;
    return true;
  }
  next(scope) {
    if (!this.page || this.page.scope !== scope?.key || this.pending || this.page.last >= this.page.lines) return Promise.resolve(false);
    return this.load(scope, this.page.last + 1, [...this.history, this.page.first]);
  }
  previous(scope) {
    if (!this.history.length || this.page?.scope !== scope?.key || this.pending) return Promise.resolve(false);
    return this.load(scope, this.history.at(-1), this.history.slice(0, -1));
  }
  close() { this.closed = true; this.intent++; this.pending = false; }
}
