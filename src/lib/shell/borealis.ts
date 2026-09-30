/** Offline-rendered curtain motion: a video decoder owns every frame.
 * No application animation loop, image warp or per-frame light calculations. */
export function motionRate(speed: number): number {
  return .5 + Math.max(0, Math.min(100, Number.isFinite(speed) ? speed : 50)) / 100;
}
export function motionAllowed(hidden: boolean, focused: boolean, reduced: boolean): boolean {
  return !hidden && focused && !reduced;
}
export interface BorealisReview { time?: number; reducedMotion?: boolean; source?: string }

/** Local, muted loop. Hiding/unmounting releases work; accessibility overrides speed. */
export function mountBorealis(video: HTMLVideoElement, review: BorealisReview = {}): () => void {
  const reduced = matchMedia('(prefers-reduced-motion: reduce)');
  let disposed = false, failed = false, loaded = false, playRejected = false, reportedPlayFailure = false;
  function still() { return reduced.matches || review.reducedMotion === true; }
  function allowed() { return motionAllowed(document.hidden, document.hasFocus(), still()) && review.time === undefined; }
  function sync() {
    if (disposed || failed) return;
    if (still()) {
      video.pause(); video.style.opacity = '0'; video.dataset.motion = 'reduced';
      return;
    }
    if (!loaded && (allowed() || review.time !== undefined)) {
      loaded = true; video.src = review.source ?? '/backgrounds/borealis-loop.mp4'; video.load();
    }
    if (allowed()) {
      video.dataset.motion = 'starting';
      void video.play().then(() => {
        if (disposed || !allowed()) video.pause();
      }).catch((error: unknown) => {
        if (disposed || !allowed()) return;
        playRejected = true;
        video.style.opacity = '0';
        video.dataset.motion = 'fallback';
        if (!reportedPlayFailure) {
          reportedPlayFailure = true;
          console.warn('Borealis playback could not start:', error instanceof DOMException ? error.name : 'unknown');
        }
      });
    } else { video.pause(); video.dataset.motion = 'paused'; }
  }
  function metadata() {
    if (review.time !== undefined && !still()) video.currentTime = Math.max(0, Math.min(review.time, Math.max(0, video.duration - .05)));
  }
  function frame() { if (!still() && !failed && !playRejected) video.style.opacity = '1'; }
  function playing() { if (allowed()) { playRejected = false; video.dataset.motion = 'running'; frame(); } else video.pause(); }
  function pause() { if (!still()) video.dataset.motion = 'paused'; }
  function fail() { failed = true; video.pause(); video.style.opacity = '0'; video.dataset.motion = 'fallback'; }
  video.addEventListener('loadedmetadata', metadata);
  video.addEventListener('loadeddata', frame); video.addEventListener('seeked', frame);
  video.addEventListener('playing', playing); video.addEventListener('pause', pause); video.addEventListener('error', fail);
  document.addEventListener('visibilitychange', sync);
  window.addEventListener('focus', sync); window.addEventListener('blur', sync);
  reduced.addEventListener('change', sync); sync();
  return () => {
    disposed = true;
    video.removeEventListener('loadedmetadata', metadata);
    video.removeEventListener('loadeddata', frame); video.removeEventListener('seeked', frame);
    video.removeEventListener('playing', playing); video.removeEventListener('pause', pause); video.removeEventListener('error', fail);
    document.removeEventListener('visibilitychange', sync);
    window.removeEventListener('focus', sync); window.removeEventListener('blur', sync);
    reduced.removeEventListener('change', sync);
    video.pause(); video.removeAttribute('src'); video.load();
  };
}
