import assert from 'node:assert/strict';
import { it } from 'node:test';
import { auroraCreationCopy } from './creationCopy.ts';

it('creation help follows the native compatible release and does not invent unavailable versions', () => {
  const release = { available: true, version: '2.1.5', loaderVersion: '0.19.5', reason: 'Compatible' };
  assert.match(auroraCreationCopy(release), /Aurora Client 2\.1\.5/);
  assert.match(auroraCreationCopy({ ...release, version: '3.0.0' }), /Aurora Client 3\.0\.0/);
  assert.equal(auroraCreationCopy({ ...release, available: false, version: null, reason: 'Unsupported selection' }), 'Unsupported selection');
  assert.equal(auroraCreationCopy(null), 'Checking Aurora compatibility…');
});
