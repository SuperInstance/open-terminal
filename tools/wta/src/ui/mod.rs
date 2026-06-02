mod auth;
pub(crate) mod card;
pub(crate) mod chat;
mod command_popup;
mod debug_panel;
mod input;
mod layout;
mod permission;
mod recommendations;
pub mod agents_view;
pub mod setup;
pub mod shimmer;

#[cfg(feature = "math-tools")]
pub mod entropy_bar;

pub use shimmer::CYCLE_FRAMES as ACTIVITY_CYCLE_FRAMES;
pub use command_popup::PopupState;
pub use layout::render;

#[cfg(feature = "math-tools")]
pub mod agent_disagreement;
