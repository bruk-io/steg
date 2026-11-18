//! The one definition of steg's API (decision-1).
//!
//! [`description`] is the single source every consumer reads: the
//! dispatch task's server validates and dispatches from it, a future CLI
//! (TASK-1.49) builds its whole command tree from it, the reference docs
//! are rendered from it, and a Python plugin gets it as JSON. Nothing
//! downstream names a method, a parameter or a flag of its own.
//!
//! Every method doc-2 section 8 lists for the first release is here —
//! `version` and `completion` are not API methods (doc-2 section 2) and are
//! not. Which methods the kernel can serve today and which cannot yet is a
//! dispatch question, carried on TASK-1.43's board task for the task that
//! builds dispatch; this module describes all 42 regardless, each with the
//! shape its `returns` promises, because `fields` alone already answers
//! R37 whether or not a Rust type backs it yet (`SessionRecord` and
//! `SettingRecord` are the ones that do, in `crate::record`, since
//! `session.list`/`session.show` and `setting.list`/`setting.show` are
//! the only methods whose record the kernel can fill today).
//!
//! Grouped by noun, as section 8 is, so a method is easy to find and a
//! review against that document is a direct comparison.

use super::shapes::Description;

mod api;
mod client;
mod errors;
mod event;
mod pane;
mod server;
mod session;
mod setting;
mod shared;
mod tab;

use api::api_methods;
use client::client_methods;
use errors::errors;
use event::{event_methods, events};
use pane::pane_methods;
use server::server_methods;
use session::session_methods;
use setting::setting_methods;
use shared::VERSION;
use tab::tab_methods;

/// The whole API, as data.
#[must_use]
pub fn description() -> Description {
    let events = events();
    let event_names: Vec<String> = events.iter().map(|event| event.name.clone()).collect();

    let mut methods = Vec::new();
    methods.extend(session_methods());
    methods.extend(tab_methods());
    methods.extend(pane_methods());
    methods.extend(server_methods());
    methods.extend(event_methods(&event_names));
    methods.extend(client_methods());
    methods.extend(setting_methods());
    methods.extend(api_methods());

    Description {
        api_version: VERSION,
        methods,
        events,
        errors: errors(),
    }
}

#[cfg(test)]
mod tests;
