pub mod comm;
pub mod components;
pub mod ser;

pub use comm::*;
pub use components::*;

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 9870;

// FIXME Most event should only reference the element id, but
// they don't arrive in a meaningful order so it's easier for
// now to send all the needed data every time.
// Maybe a cleaner solution would simply be to send the whole
// topology every time there's a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    NewElement(Element),
    ChangeElementState {
        element: Element,
        state: State,
    },
    AddChildElement {
        child: Element,
        parent: Element,
    },
    AddPad {
        pad: Pad,
        element: Element,
    },
    LinkPad {
        src_pad: Pad,
        sink_pad: Pad,
        state: State,
    },
}
