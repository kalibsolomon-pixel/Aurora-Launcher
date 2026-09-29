import assert from 'node:assert/strict';
import { before, after, it } from 'node:test';
import { createServer } from 'vite';
import { reorderWidget } from './widgets.ts';
import { playerModel, renderPlayer } from './playerModel.ts';

it('drag ordering only moves visible registered slots and preserves hidden/unknown data', () => {
  const layout = { widgets: [
    { id: 'playtime', enabled: true, size: 'small' as const },
    { id: 'future-widget', enabled: true, size: 'wide' as const },
    { id: 'session', enabled: false, size: 'small' as const },
    { id: 'recent-worlds', enabled: true, size: 'large' as const },
  ] };
  const result = reorderWidget(layout, 'playtime', 'recent-worlds');
  assert.deepEqual(result.widgets.map(widget => widget.id), ['recent-worlds', 'future-widget', 'session', 'playtime']);
  assert.deepEqual(result.widgets[1], layout.widgets[1]);
  assert.deepEqual(result.widgets[2], layout.widgets[2]);
  assert.equal(reorderWidget(layout, 'future-widget', 'playtime'), layout);
  assert.equal(reorderWidget(layout, 'playtime', 'missing'), layout);
});

it('explicit player rotation changes projection without changing validated skin data', () => {
  let pixels: Uint8ClampedArray;
  const context = { createImageData: (width: number, height: number) => ({ data: new Uint8ClampedArray(width * height * 4) }), putImageData: (image: ImageData) => { pixels = image.data; } } as unknown as CanvasRenderingContext2D;
  const model = playerModel(null), original = [...model.pixels];
  renderPlayer(context, model); const front = pixels!.slice();
  renderPlayer(context, model, 1.2);
  assert.notDeepEqual(front, pixels!);
  assert.deepEqual(model.pixels, original);
});

let server: Awaited<ReturnType<typeof createServer>>, appearance: any;
before(async () => {
  server = await createServer({server:{middlewareMode:true},logLevel:'silent'});
  ({appearance} = await server.ssrLoadModule('/src/lib/launcher/appearance.svelte.ts'));
});
after(async () => { await server?.close(); });
it('theme, accent and background writes preserve the other dimensions and failed saves preserve state', async () => {
  const root = {dataset: {},style:{removeProperty() {},setProperty() {}}};
  (globalThis as any).document = {documentElement:root};
  let persisted = {theme:'oled',background:'simple',accent:{type:'preset',id:'cyan'},themes:[],accents:[],palette:{}};
  const calls: any[] = [];
  (globalThis as any).window = {__TAURI_INTERNALS__:{invoke:async (command:string,args:any) => {
    calls.push({command,args}); persisted = {...persisted,...args.request}; return structuredClone(persisted);
  }}};
  appearance.apply(structuredClone(persisted));
  await appearance.setBackground('borealis');
  assert.equal(appearance.theme,'oled'); assert.deepEqual(appearance.accent,{type:'preset',id:'cyan'});
  await appearance.setTheme('midnight');
  assert.equal(appearance.background,'borealis');
  await appearance.setAccent({type:'preset',id:'rose'});
  assert.equal(appearance.background,'borealis'); assert.equal(appearance.theme,'midnight');
  assert(calls.every(call => call.command === 'set_appearance' && call.args.request.background === 'borealis'));
  (globalThis as any).window.__TAURI_INTERNALS__.invoke = async () => { throw {code:'config_malformed',message:'Configuration needs attention.'}; };
  await appearance.setBackground('simple');
  assert.equal(appearance.background,'borealis'); assert.match(appearance.error.message,/needs attention/);
});
