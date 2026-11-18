//! The `server` noun's methods.

use crate::description::shapes::{ErrorCode, Field, Follows, Method, Returns, ValueKind};

use super::shared::{VERSION, field, method, switch};

pub(super) fn server_fields() -> Vec<Field> {
    vec![
        field("version", ValueKind::Text, "The server's own version"),
        field(
            "uptime",
            ValueKind::Integer,
            "How long it has been running, in seconds",
        ),
        field("sessions", ValueKind::Integer, "How many sessions it holds"),
        field(
            "clients",
            ValueKind::Integer,
            "How many clients are connected",
        ),
    ]
}

pub(super) fn server_methods() -> Vec<Method> {
    vec![
        Method {
            name: method("server", "show"),
            summary: "The server's own state".into(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Record {
                fields: server_fields(),
            },
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("server", "start"),
            summary: "Start the server, with no clients attached".into(),
            details: "Stays up with no clients. With on_demand, it is the kind a client \
                      starts when it finds no server: it stops itself after 30 seconds with \
                      no sessions and no clients (doc-8 R99). `steg server start \
                      --on-demand` is what the sdk's local-socket provider runs (TASK-1.70)."
                .into(),
            params: vec![switch(
                "on_demand",
                "Start as an on-demand server, which stops itself after 30 seconds with no \
                 sessions and no clients (doc-8 R99), rather than staying up until told to \
                 stop",
            )],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest],
            follows: Follows::Nothing,
            since: VERSION,
        },
        Method {
            name: method("server", "stop"),
            summary: "Stop the server".into(),
            details: String::new(),
            params: vec![switch("force", "Stop it even though sessions exist")],
            returns: Returns::Nothing,
            errors: vec![ErrorCode::BadRequest, ErrorCode::ProgramRunning],
            follows: Follows::Nothing,
            since: VERSION,
        },
    ]
}
