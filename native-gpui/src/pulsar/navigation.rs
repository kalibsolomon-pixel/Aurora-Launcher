//! Scroll positions are presentation state, captured only during UI events/renders.
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct ScrollMemory {
    current: Option<String>,
    offsets: BTreeMap<String, f32>,
}
impl ScrollMemory {
    pub fn transition(&mut self, key: String, offset: f32) -> Option<f32> {
        if self.current.as_ref() == Some(&key) {
            return None;
        }
        if let Some(previous) = self.current.replace(key.clone()) {
            self.offsets.insert(previous, offset.min(0.));
        }
        Some(self.offsets.get(&key).copied().unwrap_or(0.))
    }
}
impl super::Pulsar {
    pub(super) fn remember_scroll(&mut self) {
        let Some(scroll) = &self.page_scroll else {
            return;
        };
        let key = if self.compare_open {
            "compare".to_owned()
        } else if let Some(r) = &self.detail_request {
            format!(
                "detail:{}:{}:{:?}:{:?}:{}",
                r.key.instance_id, r.key.fight_id, r.participant, r.exchange_id, self.detail_tab
            )
        } else {
            format!("top:{}", self.top_tab)
        };
        if let Some(y) = self
            .scroll_memory
            .transition(key, scroll.offset().y.as_f32())
        {
            scroll.set_offset(gpui::point(gpui::px(0.), gpui::px(y)));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tabs_detail_and_compare_keep_independent_scroll() {
        let mut m = ScrollMemory::default();
        assert_eq!(m.transition("overview".into(), 0.), Some(0.));
        assert_eq!(m.transition("offense".into(), -240.), Some(0.));
        assert_eq!(m.transition("detail:flow".into(), -500.), Some(0.));
        assert_eq!(m.transition("offense".into(), -100.), Some(-500.));
        assert_eq!(m.transition("overview".into(), -520.), Some(-240.));
        assert_eq!(m.transition("overview".into(), -250.), None);
    }
}
