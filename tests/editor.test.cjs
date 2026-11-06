const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const { createCloseHandler } = require('../frontend/close.js');

async function app(customInvoke) {
  const elements = new Map();
  function element(id) {
    if (!elements.has(id)) elements.set(id, {
      value: '', open: false, disabled: false, textContent: '', innerHTML: '', style: {}, dataset: {}, events: {},
      classList: { toggle() {} },
      addEventListener(name, fn) { this.events[name] = fn; },
      replaceChildren() {}, append() {}, focus() {}, remove() {},
      setAttribute() {}, getAttribute() { return 'true'; },
      showModal() { this.open = true; }, close() { this.open = false; },
    });
    return elements.get(id);
  }
  let closeHandler;
  const invoke = customInvoke || (async (command, args) => {
    if (command === 'preview') return { timeline: structuredClone(args.timeline), svg: '<svg/>' };
    throw Error(command);
  });
  const context = vm.createContext({
    structuredClone, createCloseHandler, setTimeout() {},
    localStorage: { getItem() {}, setItem() {} },
    document: {
      getElementById: element, createElement: () => element(Symbol()),
      body: { classList: { toggle() {} } }, addEventListener() {},
      querySelectorAll: () => [...elements.values()],
    },
    window: { __TAURI__: { core: { invoke }, window: { getCurrentWindow: () => ({
      onCloseRequested: async handler => { closeHandler = handler; }, destroy: async () => {},
    }) } } },
  });
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../frontend/app.js'), 'utf8'), context);
  await new Promise(resolve => setImmediate(resolve));
  return { element, run: code => vm.runInContext(code, context), close: () => closeHandler({ preventDefault() {} }) };
}
const submit = { preventDefault() {} };

test('saving an unchanged imported color preserves its transparency', async () => {
  const ui = await app();
  ui.run("timeline.blocks.push({title:'Existing',subtitle:null,start_year:2020,end_year:2021,color:[80,40,20,128]});edit(0)");
  ui.element('title').value = 'Renamed';
  await ui.element('block-form').onsubmit(submit);
  assert.equal(ui.run('JSON.stringify(timeline.blocks[0].color)'), '[80,40,20,128]');
});
test('double submit adds one period and prevents cancelling a pending save', async () => {
  let finish; let requests = 0;
  const ui = await app(async (command, args) => {
    assert.equal(command, 'preview'); requests++;
    const result = { timeline: structuredClone(args.timeline), svg: '<svg/>' };
    if (requests === 1) return result;
    return new Promise(resolve => { finish = () => resolve(result); });
  });
  ui.run('edit()'); ui.element('title').value = 'New entry';
  const pending = ui.element('block-form').onsubmit(submit);
  assert.equal(ui.element('title').disabled, true);
  ui.run('closeForm()'); assert.equal(ui.element('block-dialog').open, true);
  await ui.element('block-form').onsubmit(submit); assert.equal(requests, 2);
  finish(); await pending;
  assert.equal(ui.run('timeline.blocks.length'), 1);
  assert.equal(ui.element('title').disabled, false);
});
test('open consumes the prepared preview without a second failing render', async () => {
  let previews = 0;
  const ui = await app(async (command, args) => {
    if (command === 'preview') { previews++; return { timeline: args.timeline, svg: '<svg/>' }; }
    if (command === 'open_project') return { name: 'Loaded.json', timeline: { start_year: 2000, end_year: 2010, blocks: [] }, svg: '<svg id="loaded"/>' };
    throw Error(command);
  });
  await ui.element('open').onclick();
  assert.equal(previews, 1); assert.equal(ui.element('paper').innerHTML, '<svg id="loaded"/>');
  assert.equal(ui.run('timeline.start_year'), 2000); assert.equal(ui.run('dirty'), false);
});
test('an untouched form is not an unsaved change but a typed description is', async () => {
  const ui = await app(); ui.run('edit()');
  assert.equal(ui.run('hasFormChanges()'), false);
  ui.element('description').value = 'Unsaved text';
  assert.equal(ui.run('hasFormChanges()'), true);
});
test('startup failure still installs close protection and blocks edits', async () => {
  const ui = await app(async () => { throw Error('Font unavailable'); });
  ui.run('edit()'); assert.equal(ui.element('block-dialog').open, false);
  assert.match(ui.element('status').textContent, /Font unavailable/);
  await ui.close();
});
