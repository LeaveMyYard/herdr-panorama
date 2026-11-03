export async function resolve(specifier, context, nextResolve) {
  if (specifier === 'herdr') return { url: new URL('./herdr-fixture.mjs', import.meta.url).href, shortCircuit: true };
  return nextResolve(specifier, context);
}
