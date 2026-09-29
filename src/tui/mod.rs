//! TUI module for interactive terminal user interface

pub mod app;
pub mod editor;
pub mod keys;
pub mod line_edit;
pub mod list;
pub mod operations;
pub mod search;
pub mod selection;
pub mod state;
pub mod ui;

pub use app::TuiApp;
