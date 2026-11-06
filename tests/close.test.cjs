const { test } = require('node:test');
const assert = require('node:assert/strict');
const { createCloseHandler } = require('../frontend/close.js');
const capability = require('../capabilities/default.json');

function setup(overrides = {}) {
  const calls = { destroyed: 0, prompted: 0, prevented: 0, messages: [] };
  const handler = createCloseHandler({
    windowHandle: { destroy: async () => { calls.destroyed++; } },
    hasUnsavedChanges: () => false,
    isBusy: () => false,
    confirmDiscard: async () => { calls.prompted++; return true; },
    report: (...args) => calls.messages.push(args),
    ...overrides,
  });
  return { calls, close: () => handler({ preventDefault: () => { calls.prevented++; } }) };
}
test('clean close destroys once without prompting, with the required permission', async () => {
  assert.ok(capability.permissions.includes('core:window:allow-destroy'));
  const { calls, close } = setup(); await close();
  assert.equal(calls.destroyed, 1); assert.equal(calls.prompted, 0); assert.equal(calls.prevented, 1);
});
test('unsaved changes require approval', async () => {
  const { calls, close } = setup({ hasUnsavedChanges: () => true }); await close();
  assert.equal(calls.prompted, 1); assert.equal(calls.destroyed, 1);
});
test('cancel keeps the window open and allows retry', async () => {
  let accepted = false;
  const { calls, close } = setup({ hasUnsavedChanges: () => true, confirmDiscard: async () => accepted });
  await close(); assert.equal(calls.destroyed, 0);
  accepted = true; await close(); assert.equal(calls.destroyed, 1);
});
test('repeated clicks do not open duplicate confirmations', async () => {
  let approve; let prompts = 0;
  const { calls, close } = setup({ hasUnsavedChanges: () => true, confirmDiscard: () => { prompts++; return new Promise(resolve => { approve = resolve; }); } });
  const pending = close(); await close(); assert.equal(prompts, 1);
  approve(true); await pending; assert.equal(calls.destroyed, 1);
});
test('failed close reports an error and can be retried', async () => {
  let attempts = 0;
  const { calls, close } = setup({ windowHandle: { destroy: async () => { if (++attempts === 1) throw new Error('Denied'); } } });
  await close(); assert.match(calls.messages[0][0], /Denied/); assert.equal(calls.messages[0][1], true);
  await close(); assert.equal(attempts, 2);
});
test('active operation is preserved and close works after completion', async () => {
  let busy = true; const { calls, close } = setup({ isBusy: () => busy });
  await close(); assert.equal(calls.destroyed, 0); assert.equal(calls.messages.length, 1);
  busy = false; await close(); assert.equal(calls.destroyed, 1);
});
