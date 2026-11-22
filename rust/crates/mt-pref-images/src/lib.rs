//! Images & uploader preferences pane (`prefs.pane.images`).
//!
//! Covers the Images category page: Pandoc (from `generalGroups.images`),
//! insert action, folder settings, and uploader. User-data keys
//! (`imageFolderPath`, `currentUploader`, `cliScript`) are not in the
//! preference schema; the pane keeps them beside the bridge snapshot.

mod model;
mod view;

pub use model::{
    Choice, DetectionState, ImagesModel, PandocSwitch, PersistDecision, PersistGate,
    UserDataKeys, draft_still_matches, extract_user_data, folder_path_valid,
    pandoc_switch_state, relative_directory_name_valid,
};
pub use view::{HostActionPublisher, ImagesPane, PersistPublisher, PreferencePublisher};
