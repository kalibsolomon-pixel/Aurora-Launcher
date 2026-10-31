//! One render in flight, with a single implicit latest-pose slot. All gate
//! transitions belong to the UI thread; the CPU job owns an immutable scene.
//! No unbounded channels, idle work, or texture copies on pointer input.
use std::sync::Arc;

use crate::api::{HeadAvatar, Texture};

#[derive(Default)]
pub struct RenderGate {
    revision: u64,
    started: u64,
    in_flight: bool,
}
impl RenderGate {
    pub fn request(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn start(&mut self) -> bool {
        if !self.ready() {
            return false;
        }
        self.started = self.revision;
        self.in_flight = true;
        true
    }
    pub fn finish(&mut self) {
        self.in_flight = false;
    }
    pub fn ready(&self) -> bool {
        !self.in_flight && self.started != self.revision
    }
}

#[derive(Clone)]
pub struct Scene {
    pub skin: Option<Arc<HeadAvatar>>,
    pub cape: Option<Arc<Texture>>,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub layers: bool,
}
impl Scene {
    pub fn render(&self) -> Vec<u8> {
        crate::player::render(
            self.skin.as_deref(),
            self.cape.as_deref(),
            self.yaw,
            self.pitch,
            self.zoom,
            self.layers,
        )
    }
    /// A completed pose can be shown while dragging, but never flash a skin,
    /// cape or layer setting that was replaced while the worker was running.
    pub fn same_appearance(&self, other: &Self) -> bool {
        self.skin.as_ref().map(Arc::as_ptr) == other.skin.as_ref().map(Arc::as_ptr)
            && self.cape.as_ref().map(Arc::as_ptr) == other.cape.as_ref().map(Arc::as_ptr)
            && self.layers == other.layers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rapid_input_keeps_one_job_and_one_latest_pose_then_stops() {
        let mut gate = RenderGate::default();
        assert!(!gate.start());
        gate.request();
        assert!(gate.start());
        for _ in 0..10_000 {
            gate.request();
            assert!(!gate.start());
        }
        gate.finish();
        assert!(gate.start());
        assert_eq!(gate.started, 10_001);
        gate.finish();
        assert!(!gate.start(), "idle must schedule no work");
        gate.request();
        assert!(gate.start());
    }
    #[test]
    fn replaced_textures_and_layers_reject_stale_frames_but_rotation_does_not() {
        let a = Scene {
            skin: None,
            cape: None,
            yaw: 0.,
            pitch: 0.,
            zoom: 1.,
            layers: true,
        };
        let mut b = a.clone();
        b.yaw = 2.;
        assert!(a.same_appearance(&b));
        b.layers = false;
        assert!(!a.same_appearance(&b));
        b = a.clone();
        b.cape = Some(Arc::new(Texture {
            width: 64,
            height: 32,
            rgba: vec![0; 8192],
        }));
        assert!(!a.same_appearance(&b));
    }
}
