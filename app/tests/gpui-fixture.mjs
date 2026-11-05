// Stands in for the native element builders: an element records its calls and children (`kids`), so a
// test can read what a mod described. Every other builder method just records and chains.
function element(kind, args = []) {
  const node = { kind, args, calls: {}, kids: [] };
  const self = new Proxy(node, {
    get(target, name) {
      if (name in target) return target[name];
      if (typeof name === 'symbol') return undefined;
      return (...values) => {
        if (name === 'when') return values[0] ? values[1](self) : self;
        if (name === 'child') target.kids.push(values[0]);
        else if (name === 'children') target.kids.push(...values[0]);
        else target.calls[name] = values;
        return self;
      };
    },
  });
  return self;
}
export const div = () => element('div'), h_flex = () => element('h_flex'), v_flex = () => element('v_flex');
export const svg = path => element('svg', [path]), image = path => element('image', [path]);
export class View {}
