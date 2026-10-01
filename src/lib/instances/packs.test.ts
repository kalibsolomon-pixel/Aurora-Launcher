import assert from 'node:assert/strict';
import { it } from 'node:test';
import type { ContentEntry, ContentManagement, ProviderRecord } from '$lib/backend';
import {
  packRemovalPlan,
  packStateLabel,
  packToggleUnavailableReason,
  confirmedLocalRemoval,
  visiblePacks,
  packSourceLabel,
} from './packs.ts';

const baseManagement: ContentManagement = {
  canRemove: true,
  removalPath: 'localFile',
  removalBlockedReason: null,
  canToggle: true,
  active: false,
  activationManagedInGame: false,
  toggleBlockedReason: null,
};

function entry(overrides: Partial<ContentEntry> & { management?: Partial<ContentManagement> }): ContentEntry {
  return {
    entryId: 'entry-1',
    contentType: 'resourcePack',
    fileName: 'Pack.zip',
    displayName: 'Pack',
    fileType: 'zip',
    sizeBytes: 10,
    modifiedUnixMillis: null,
    ownership: 'userManaged',
    sha256: null,
    provenance: null,
    description: null,
    packFormat: null,
    warnings: [],
    ...overrides,
    management: { ...baseManagement, ...(overrides.management ?? {}) },
  };
}

function provenance(origin: ProviderRecord['origin']): ProviderRecord {
  return {
    provider: 'modrinth',
    projectId: 'project',
    versionId: 'version',
    fileId: 'file',
    contentType: 'resourcePack',
    fileName: 'Pack.zip',
    sha256: 'a'.repeat(64),
    displayVersion: '1.2.3',
    compatibility: { minecraftVersions: ['1.21.11'], loader: null, environment: 'client' },
    dependencies: [],
    explicitlyRetained: true,
    requires: [],
    origin,
    installedAtUnixSeconds: null,
    pinned: false,
    updateChannel: 'stable',
  } as ProviderRecord;
}

it('local unmanaged packs route to the entry-level file removal', () => {
  const plan = packRemovalPlan(entry({}));
  assert.equal(plan?.route, 'local');
  assert.equal(plan?.blockedReason, null);
  assert.equal(packSourceLabel(entry({})), 'Local · ZIP');
});

it('verified managed packs route to the provider graph lifecycle', () => {
  const plan = packRemovalPlan(
    entry({ ownership: 'providerManaged', management: { removalPath: 'providerGraph' } }),
  );
  assert.equal(plan?.route, 'provider');
  assert.equal(plan?.blockedReason, null);
});

it('blocked entries keep their trash disabled with the real reason instead of Unavailable', () => {
  const folder = entry({
    fileName: 'Folder Pack',
    fileType: 'directory',
    management: {
      canRemove: false,
      removalPath: 'blocked',
      removalBlockedReason: 'Folder packs are left untouched; remove them from the content folder directly.',
    },
  });
  const plan = packRemovalPlan(folder);
  assert.equal(plan?.route, 'local');
  assert.equal(plan?.blockedReason, 'Folder packs are left untouched; remove them from the content folder directly.');
});

it('resource packs report Enabled/Disabled from the backend activation state', () => {
  assert.equal(packStateLabel(entry({ management: { active: true } })), 'Enabled');
  assert.equal(packStateLabel(entry({ management: { active: false } })), 'Disabled');
});

it('shaders truthfully report in-game activation without a fake toggle', () => {
  const shader = entry({
    contentType: 'shaderPack',
    management: {
      canToggle: false,
      active: null,
      activationManagedInGame: true,
      removalPath: 'providerGraph',
    },
  });
  assert.equal(packStateLabel(shader), 'Activation managed in-game');
  assert.equal(packToggleUnavailableReason(shader), 'Shader packs are activated in-game with a compatible shader loader.');
  assert.equal(packRemovalPlan(shader)?.route, 'provider');
});

it('unreadable options disable activation with their actual reason', () => {
  const pack = entry({
    management: {
      canToggle: false,
      active: null,
      toggleBlockedReason: "Minecraft's options.txt could not be read safely; pack activation is unavailable.",
    },
  });
  assert.equal(packStateLabel(pack), 'Activation unavailable');
  assert.equal(
    packToggleUnavailableReason(pack),
    "Minecraft's options.txt could not be read safely; pack activation is unavailable.",
  );
});

it('a toggleable pack never exposes an unavailable reason', () => {
  assert.equal(packToggleUnavailableReason(entry({ management: { active: true } })), null);
});

it('recovered provenance stays visible while capabilities stay normal', () => {
  const recovered = entry({
    ownership: 'providerManaged',
    provenance: provenance('recovered'),
    management: { removalPath: 'providerGraph', active: true },
  });
  assert.equal(recovered.provenance?.origin, 'recovered');
  assert.equal(packRemovalPlan(recovered)?.route, 'provider');
  assert.equal(packStateLabel(recovered), 'Enabled');
  assert.equal(packSourceLabel(recovered), 'Managed · modrinth · 1.2.3');
});

it('confirmed local removals re-check the current snapshot capability', () => {
  const pack = entry({});
  assert.equal(confirmedLocalRemoval({ entryId: 'entry-1', displayName: 'Pack', fileName: 'Pack.zip' }, [pack]), 'entry-1');
  const stale = entry({ management: { canRemove: false, removalPath: 'blocked', removalBlockedReason: 'nope' } });
  assert.equal(confirmedLocalRemoval({ entryId: 'entry-1', displayName: 'Pack', fileName: 'Pack.zip' }, [stale]), null);
  assert.equal(confirmedLocalRemoval(null, [pack]), null);
  assert.equal(confirmedLocalRemoval({ entryId: 'missing', displayName: 'x', fileName: 'x' }, [pack]), null);
});

it('visiblePacks filters by activation, warnings, management and search', () => {
  const enabled = entry({ entryId: 'a', displayName: 'Alpha', management: { active: true } });
  const disabled = entry({ entryId: 'b', displayName: 'Beta', management: { active: false } });
  const warned = entry({
    entryId: 'c',
    displayName: 'Gamma',
    management: { active: true },
    warnings: [{ code: 'x', message: 'warn' } as never],
  });
  const managed = entry({
    entryId: 'd',
    displayName: 'Delta',
    ownership: 'providerManaged',
    management: { active: true },
  });
  const all = [enabled, disabled, warned, managed];
  assert.deepEqual(visiblePacks(all, '', 'all').map((item) => item.entryId), ['a', 'b', 'c', 'd']);
  assert.deepEqual(visiblePacks(all, '', 'enabled').map((item) => item.entryId), ['a', 'c', 'd']);
  assert.deepEqual(visiblePacks(all, '', 'disabled').map((item) => item.entryId), ['b']);
  assert.deepEqual(visiblePacks(all, '', 'warnings').map((item) => item.entryId), ['c']);
  assert.deepEqual(visiblePacks(all, '', 'managed').map((item) => item.entryId), ['d']);
  assert.deepEqual(visiblePacks(all, 'alpha', 'all').map((item) => item.entryId), ['a']);
  assert.deepEqual(visiblePacks(all, 'modrinth', 'all').map((item) => item.entryId), []);
});
