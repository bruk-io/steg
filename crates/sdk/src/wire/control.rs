//! The control envelope: a request carries an id and is answered by its
//! result or its error, matched by that same id; an event carries neither,
//! so a long-lived subscription never blocks a request on the same
//! connection (decision-2 section 3).
//!
//! Built on TASK-1.43's `description::{Request, Response}` rather than
//! beside them: `Response` is already `Ok(Value) | Error(ApiError)`, so
//! "its result or its error, matched by id" needs no new type here.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::description::{Request, Response};

/// One message on the control stream (stream 0), or a screen-mode
/// attachment's update (a `Control::Event`-shaped frame on the
/// attachment's own stream - decision-2 section 3; no new [`super::Kind`]
/// is needed for that, only a non-zero stream id).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Control {
    Request { id: u64, request: Request },
    Reply { id: u64, response: Response },
    Event { name: String, body: Value },
}

impl Control {
    /// # Errors
    /// When the value cannot be serialised.
    pub fn to_payload(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// # Errors
    /// When the bytes are not a control message.
    pub fn from_payload(payload: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(payload)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::Control;
    use crate::description::{ApiError, ErrorCode, Request, Response};

    #[test]
    fn a_request_round_trips_through_its_payload() {
        let mut params = Map::new();
        params.insert("direction".into(), json!("right"));
        let control = Control::Request {
            id: 7,
            request: Request {
                method: "pane.split".into(),
                params,
            },
        };
        let payload = control.to_payload().unwrap();
        let Control::Request { id, request } = Control::from_payload(&payload).unwrap() else {
            panic!("expected a request");
        };
        assert_eq!(id, 7);
        assert_eq!(request.method, "pane.split");
        assert_eq!(request.params.get("direction"), Some(&json!("right")));
    }

    #[test]
    fn a_reply_carries_the_result_matched_by_id() {
        let ok = Control::Reply {
            id: 1,
            response: Response::Ok(Value::String("pane-1".into())),
        };
        let payload = ok.to_payload().unwrap();
        let Control::Reply { id, response } = Control::from_payload(&payload).unwrap() else {
            panic!("expected a reply");
        };
        assert_eq!(id, 1);
        assert!(matches!(response, Response::Ok(Value::String(s)) if s == "pane-1"));
    }

    #[test]
    fn a_reply_carries_the_error_matched_by_id() {
        let error = Control::Reply {
            id: 2,
            response: Response::Error(ApiError::new(ErrorCode::NoSuchTarget, "no such pane")),
        };
        let payload = error.to_payload().unwrap();
        let Control::Reply { id, response } = Control::from_payload(&payload).unwrap() else {
            panic!("expected a reply");
        };
        assert_eq!(id, 2);
        let Response::Error(api_error) = response else {
            panic!("expected an error response");
        };
        assert_eq!(api_error.code, ErrorCode::NoSuchTarget);
    }

    #[test]
    fn an_event_carries_no_id() {
        let event = Control::Event {
            name: "pane.closed".into(),
            body: json!({"id": "00000001"}),
        };
        let payload = event.to_payload().unwrap();
        let Control::Event { name, body } = Control::from_payload(&payload).unwrap() else {
            panic!("expected an event");
        };
        assert_eq!(name, "pane.closed");
        assert_eq!(body, json!({"id": "00000001"}));
        assert!(!String::from_utf8_lossy(&payload).contains("\"id\":1"));
    }
}
