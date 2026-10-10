import assert from 'node:assert/strict';
import { it } from 'node:test';
import { libraryEntries, isCurrentSkin, validSkinName } from './skinLibrary.ts';
const skins = [
  {id:'a',name:'Aurora',model:'classic' as const,importedAt:1,sha256:'a'.repeat(64),favorite:true},
  {id:'b',name:'Boreal',model:'slim' as const,importedAt:4,sha256:'b'.repeat(64),favorite:false},
  {id:'c',name:'Amethyst',model:'slim' as const,importedAt:4,sha256:'a'.repeat(64),favorite:true},
];
it('library search, favorites and deterministic recent sorting do not mutate native records',()=>{
  assert.deepEqual(libraryEntries(skins,'',false,'recent').map(p=>p.id),['b','c','a']);
  assert.deepEqual(libraryEntries(skins,' AUR ',true,'name').map(p=>p.id),['a']);
  assert.deepEqual(libraryEntries(skins,'',true,'name').map(p=>p.id),['c','a']);
  assert.deepEqual(skins.map(p=>p.id),['a','b','c']);
});
it('current markers require exact content and model rather than a name or opposite model',()=>{
  const avatar = {sha256:skins[0].sha256,model:'classic'} as any;
  assert.equal(isCurrentSkin(skins[0],avatar),true);
  assert.equal(isCurrentSkin(skins[2],avatar),false);
  assert.equal(isCurrentSkin(skins[0],null),false);
});
it('names use the native UTF-8 bound and reject control characters',()=>{
  assert.ok(validSkinName('界'.repeat(26))); assert.ok(validSkinName('x'.repeat(80)));
  for(const name of ['', '  ', '界'.repeat(27), 'x'.repeat(81), 'bad\nname']) assert.equal(validSkinName(name),false);
});
