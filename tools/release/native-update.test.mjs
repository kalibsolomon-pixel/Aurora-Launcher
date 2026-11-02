import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createNativePayload,createNativeEnvelope} from './native-update.mjs';
const fixture=JSON.parse(readFileSync(new URL('../../core/src/updates/fixtures/native-test.json',import.meta.url)));
const bytes=Buffer.from(fixture.installer,'base64');
const payloadBytes=Buffer.from(fixture.valid.payload,'base64');
const offer=JSON.parse(payloadBytes);
test('native preparation preserves exact signed artifact identity and channel',()=>{
  assert.deepEqual(createNativePayload(bytes,offer.installerSignature,fixture.key,'1.4.3','1.4.2','a'.repeat(40)),offer);
  assert.deepEqual(createNativeEnvelope(payloadBytes,fixture.valid.signature,fixture.key),fixture.valid);
});
test('native preparation refuses modified bytes or signature/version confusion',()=>{
  assert.throws(()=>createNativePayload(Buffer.from('changed'),offer.installerSignature,fixture.key,'1.4.3','1.4.2','a'.repeat(40)));
  assert.throws(()=>createNativePayload(bytes,offer.installerSignature,fixture.key,'1.4.4','1.4.2','a'.repeat(40)));
  assert.throws(()=>createNativeEnvelope(Buffer.from(payloadBytes.toString().replace('native-preview','stable')),fixture.valid.signature,fixture.key));
});
