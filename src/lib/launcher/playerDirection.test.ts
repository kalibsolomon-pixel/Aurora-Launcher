import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { it } from 'node:test';
import {
  playerModel,
  renderPlayer,
  arrowYawDelta,
  PLAYER_DEFAULT_YAW,
  PLAYER_ROTATE_STEP,
  PLAYER_RENDER_WIDTH,
  PLAYER_RENDER_HEIGHT,
} from './playerModel.ts';

type Frame = { data: Uint8ClampedArray };

function renderAt(yaw: number): Frame {
  let captured: Frame | undefined;
  const context = {
    createImageData: (width: number, height: number) => ({ data: new Uint8ClampedArray(width * height * 4) }),
    putImageData: (image: ImageData) => { captured = { data: image.data }; },
  } as unknown as CanvasRenderingContext2D;
  renderPlayer(context, playerModel(null), yaw);
  return captured!;
}

/**
 * Visual-direction probe: the default skin's eye pixels exist only on the
 * player's FRONT face, so their horizontal centroid is where the displayed
 * player is looking. This measures the rendered result, never the raw yaw
 * sign — the renderer's coordinate system is the source of truth.
 */
function frontFaceCentroidX(frame: Frame): number {
  let sum = 0, count = 0;
  for (let index = 0; index < frame.data.length; index += 4) {
    const [r, g, b, a] = [frame.data[index], frame.data[index + 1], frame.data[index + 2], frame.data[index + 3]];
    if (a > 200 && Math.abs(r - 221) <= 10 && Math.abs(g - 221) <= 10 && Math.abs(b - 228) <= 10) {
      sum += (index / 4) % PLAYER_RENDER_WIDTH;
      count += 1;
    }
  }
  assert.ok(count >= 4, `expected the front face to be visible (${count} eye pixels)`);
  return sum / count;
}

it('the front face is visible at the canonical default angle', () => {
  const frame = renderAt(PLAYER_DEFAULT_YAW);
  const center = PLAYER_RENDER_WIDTH / 2;
  const centroid = frontFaceCentroidX(frame);
  // The canonical default turns the player slightly; the face stays on
  // screen and recognizable.
  assert.ok(centroid > 0 && centroid < PLAYER_RENDER_WIDTH);
  assert.ok(Math.abs(centroid - center) < PLAYER_RENDER_WIDTH / 3);
});

it('AA: the left-arrow activation rotates the displayed player visually LEFT', () => {
  const before = frontFaceCentroidX(renderAt(PLAYER_DEFAULT_YAW));
  const after = frontFaceCentroidX(renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('left')));
  assert.ok(after < before - 4, `left arrow must move the face left (before=${before}, after=${after})`);
});

it('AB: the right-arrow activation rotates the displayed player visually RIGHT', () => {
  const before = frontFaceCentroidX(renderAt(PLAYER_DEFAULT_YAW));
  const after = frontFaceCentroidX(renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('right')));
  assert.ok(after > before + 4, `right arrow must move the face right (before=${before}, after=${after})`);
});

it('AC/AD: left-then-right and right-then-left return to the original orientation', () => {
  const original = renderAt(PLAYER_DEFAULT_YAW).data.slice();
  const roundTripLeftFirst = renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('left') + arrowYawDelta('right'));
  const roundTripRightFirst = renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('right') + arrowYawDelta('left'));
  assert.deepEqual(roundTripLeftFirst.data, original);
  assert.deepEqual(roundTripRightFirst.data, original);
});

it('AE: reset restores the canonical default orientation exactly', () => {
  const original = renderAt(PLAYER_DEFAULT_YAW).data.slice();
  const rotated = renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('left') + arrowYawDelta('left') + arrowYawDelta('right'));
  assert.notDeepEqual(rotated.data, original);
  const reset = renderAt(PLAYER_DEFAULT_YAW);
  assert.deepEqual(reset.data, original);
});

it('AF: repeated arrow activations remain deterministic', () => {
  const first = renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('left')).data.slice();
  const second = renderAt(PLAYER_DEFAULT_YAW + arrowYawDelta('left')).data.slice();
  assert.deepEqual(first, second);
  const twice = renderAt(PLAYER_DEFAULT_YAW + 2 * arrowYawDelta('left'));
  // One more bounded step continues in the same visual direction.
  assert.ok(frontFaceCentroidX(twice) < frontFaceCentroidX({ data: first }));
});

it('the accepted increment and canonical default are preserved', () => {
  assert.equal(PLAYER_ROTATE_STEP, 0.35);
  assert.equal(PLAYER_DEFAULT_YAW, -0.42);
  assert.equal(arrowYawDelta('right'), -arrowYawDelta('left'));
});

it('AG: no pointer-drag rotation handlers or drag affordances exist in the preview source', () => {
  const source = readFileSync(fileURLToPath(new URL('./PlayerPreview.svelte', import.meta.url)), 'utf8');
  for (const forbidden of ['pointermove', 'pointerdown', 'on:pointer', 'setPointerCapture', 'grab', 'requestAnimationFrame']) {
    assert.ok(!source.includes(forbidden), `PlayerPreview must not contain ${forbidden}`);
  }
});

it('AI/AH: the three controls keep their accessible labels and the canvas stays decorative', () => {
  const source = readFileSync(fileURLToPath(new URL('./PlayerPreview.svelte', import.meta.url)), 'utf8');
  assert.ok(source.includes('aria-label="Rotate player left"'));
  assert.ok(source.includes('aria-label="Reset player view"'));
  assert.ok(source.includes('aria-label="Rotate player right"'));
  // The model region is a passive image; the canvas is decorative and must
  // never become a tab stop or interactive surface.
  assert.ok(source.includes('role="img"'));
  assert.ok(source.includes('aria-hidden="true"'));
  assert.ok(!source.includes('tabindex'));
});
