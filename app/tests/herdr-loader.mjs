export async function resolve(specifier, context, nextResolve) {
  if (specifier === 'herdr') return { url: new URL('./herdr-fixture.mjs', import.meta.url).href, shortCircuit: true };
  if (specifier === 'gpui-kit' || specifier === 'gpui-base') return { url: new URL('./gpui-fixture.mjs', import.meta.url).href, shortCircuit: true };
  return nextResolve(specifier, context);
}
