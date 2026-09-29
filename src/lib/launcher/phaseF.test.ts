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
  let persisted = {auroraMotionSpeed:50,theme:'oled',background:'simple',accent:{type:'preset',id:'cyan'},themes:[],accents:[],palette:{}};
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
  await appearance.setAuroraMotionSpeed(200);
  assert.equal(appearance.auroraMotionSpeed,100);
  assert.equal(appearance.background,'borealis'); assert.equal(appearance.theme,'midnight');
  await appearance.setBackground('simple'); await appearance.setTheme('oled');
  assert.equal(appearance.auroraMotionSpeed,100);
  appearance.apply(structuredClone(persisted));
  assert.equal(appearance.auroraMotionSpeed,100); assert.equal(appearance.background,'simple');
  await appearance.setBackground('borealis');
  await appearance.setAuroraMotionSpeed(-5); assert.equal(appearance.auroraMotionSpeed,0);
  await appearance.setAuroraMotionSpeed(37); assert.equal(persisted.auroraMotionSpeed,37);
  (globalThis as any).window.__TAURI_INTERNALS__.invoke = async () => { throw {code:'config_malformed',message:'Configuration needs attention.'}; };
  await appearance.setAuroraMotionSpeed(80); assert.equal(appearance.auroraMotionSpeed,37);
  await appearance.setBackground('simple');
  assert.equal(appearance.background,'borealis'); assert.match(appearance.error.message,/needs attention/);
});

it('custom and preset colors apply all seven native palette tokens and survive reload', () => {
  const tokens = new Map<string,string>();
  const root = {dataset:{} as Record<string,string>,style:{setProperty:(key:string,value:string)=>tokens.set(key,value)}};
  (globalThis as any).document={documentElement:root};
  const palette={accent:'#ff5533',accentStrong:'#d43b20',accentHover:'#df4327',accentPressed:'#c73319',accentContrast:'#ffffff',accentSoft:'rgba(255,85,51,.14)',accentOutline:'rgba(255,85,51,.55)'};
  const state={auroraMotionSpeed:37,theme:'oled',background:'borealis',accent:{type:'custom',hex:'#ff5533'},palette,themes:[],accents:[]};
  appearance.apply(state);
  assert.equal(root.dataset.accentCustom,'true'); assert.equal(root.dataset.accent,undefined);
  assert.equal(tokens.size,7); assert.equal(tokens.get('--color-accent'),'#ff5533');
  assert.equal(tokens.get('--color-accent-strong'),'#d43b20');
  appearance.apply(structuredClone(state)); assert.equal(tokens.get('--color-accent-outline'),palette.accentOutline);
  appearance.apply({...state,accent:{type:'preset',id:'cyan'},palette:{...palette,accent:'#5bd0e0',accentStrong:'#37b3c6'}});
  assert.equal(root.dataset.accent,'cyan'); assert.equal(root.dataset.accentCustom,undefined);
  assert.equal(tokens.get('--color-accent-strong'),'#37b3c6');
});
