//! Motion (design spec § Motion, T10).

use super::app::Model;

/// True while an effect is running (the app keeps drawing).
pub fn active(_m: &Model) -> bool {
    false
}
