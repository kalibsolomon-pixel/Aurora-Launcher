import assert from 'node:assert/strict';
import { it } from 'node:test';
import { WindowActivityStore, type MinimizedEvents } from './windowActivity.ts';

function harness() {
  let emit: (event: { payload: boolean }) => void = () => {};
  const connect: MinimizedEvents = async (_event, handler) => {
    emit = handler;
    return () => { emit = () => {}; };
  };
  return { connect, emit: (payload: boolean) => emit({ payload }) };
}

it('seeds subscribers with the current state and pushes native transitions', async () => {
  const { connect, emit } = harness();
  const store = new WindowActivityStore(connect);
  const seen: boolean[] = [];
  await store.initialize();
  store.subscribe(minimized => seen.push(minimized));
  assert.deepEqual(seen, [false]);
  emit(true);
  emit(true); // duplicate native events are deduplicated
  emit(false);
  assert.deepEqual(seen, [false, true, false]);
});

it('a late subscriber is seeded with the live minimized state', async () => {
  const { connect, emit } = harness();
  const store = new WindowActivityStore(connect);
  await store.initialize();
  emit(true);
  const seen: boolean[] = [];
  const stop = store.subscribe(minimized => seen.push(minimized));
  assert.deepEqual(seen, [true], 'a subscriber attached while minimized learns the current state');
  stop();
});

it('initialize connects exactly once and unsubscribed listeners are dropped', async () => {
  const { connect, emit } = harness();
  let connections = 0;
  const counting: MinimizedEvents = async (event, handler) => {
    connections += 1;
    return connect(event, handler);
  };
  const store = new WindowActivityStore(counting);
  await store.initialize();
  await store.initialize();
  assert.equal(connections, 1);
  const seen: boolean[] = [];
  const stop = store.subscribe(minimized => seen.push(minimized));
  stop();
  emit(true);
  assert.deepEqual(seen, [false], 'no transitions reach a removed listener');
});
