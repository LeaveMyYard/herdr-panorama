// Commands carry only an opaque token; nextCommand has no argument contract.
export function peekCommand(intent, action) {
  if (!['panorama', 'panorama-graph'].includes(intent?.peekCaller)
    || !/^[0-9a-f]{32}$/.test(intent?.peekToken ?? '') || !/^[0-9a-f]{32}$/.test(intent?.peekInstallId ?? '')
    || !['closed', 'transcript'].includes(action)) return null;
  return `${intent.peekCaller}.peek-${action}-${intent.peekToken}`;
}
export async function guard(scope, current, validate, act) {
  if (!scope || !current(scope)) return null;
  await validate();
  if (!current(scope)) return null;
  const result = await act();
  if (!current(scope)) return null;
  await validate();
  return current(scope) ? result : null;
}
