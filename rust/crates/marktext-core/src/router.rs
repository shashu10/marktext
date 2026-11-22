use crate::messages::{GuestToHost, HostToGuest};

/// Remembers the one pane the single GPUI canvas is showing.
///
/// `gpui_web` owns one canvas per document, so the iframe never opens a second
/// window. The host switches panes by posting `SET_PANE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRouter {
    active: String,
}

impl PaneRouter {
    pub fn new(initial: impl Into<String>) -> Self {
        Self {
            active: initial.into(),
        }
    }

    pub fn active(&self) -> &str {
        &self.active
    }

    pub fn handle(&mut self, message: &HostToGuest) -> Option<GuestToHost> {
        match message {
            HostToGuest::HostHello => Some(GuestToHost::Ready {
                pane: Some(self.active.clone()),
            }),
            HostToGuest::SetPane { pane } => {
                self.active.clone_from(pane);
                Some(GuestToHost::PaneChanged { pane: pane.clone() })
            }
            HostToGuest::Preferences { .. } => Some(GuestToHost::PreferencesApplied),
            HostToGuest::PreferencePatch { .. } => Some(GuestToHost::PreferencesApplied),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_pane_is_the_only_canvas_switch() {
        let mut router = PaneRouter::new("general");
        let ready = router.handle(&HostToGuest::HostHello).unwrap();
        assert_eq!(
            ready,
            GuestToHost::Ready {
                pane: Some("general".to_string())
            }
        );
        let changed = router
            .handle(&HostToGuest::SetPane {
                pane: "editor".to_string(),
            })
            .unwrap();
        assert_eq!(
            changed,
            GuestToHost::PaneChanged {
                pane: "editor".to_string()
            }
        );
        assert_eq!(router.active(), "editor");
    }
}
