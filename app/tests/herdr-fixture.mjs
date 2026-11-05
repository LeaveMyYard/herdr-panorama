import { readFileSync } from 'node:fs';
export const state = { home: '', execs: [], shared: [], watches: [], read: null, exec: null, values: {}, layout: { content: { mod: 'panorama' } } };
export const appHome = () => state.home;
export const readTextFile = async path => {
  if (state.read) return state.read(path);
  try { return readFileSync(path, 'utf8'); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
};
export const exec = async (command, args) => {
  state.execs.push({ command, args });
  return state.exec ? state.exec(command, args) : { code: 0, stdout: 'panorama app-feed v2\n', stderr: '' };
};
export const shared = key => state.values[key];
export const layout = () => state.layout;
export const setShared = (key, value) => { state.values[key] = value; state.shared.push({ key, value }); };
export const watchFolder = async path => new Promise(resolve => state.watches.push({ path, resolve }));
// Native world for PeekCaller: empty live lists, recorded overlays.
export const agents = () => [], terminals = () => [], panes = () => [], spaces = () => [];
export const settings = () => state.settings ?? {};
export const focused = () => null, props = () => null, theme = () => ({}), run = () => null, nextCommand = () => new Promise(() => {});
export const openOverlay = spec => { state.overlays = [...(state.overlays ?? []), spec]; return `overlay-${state.overlays.length}`; };
export const closeOverlay = () => null;
