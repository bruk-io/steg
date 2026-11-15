//! steg's API, as one value, and validating a request against it
//! (decision-1, R101, R105).

mod methods;
mod request;
mod shapes;

pub use methods::description;
pub use request::{ApiError, Request, Response, validate};
pub use shapes::{
    Description, ErrorCode, ErrorRecord, Event, Field, Method, MethodName, Param, Returns, Role,
    ValueKind, Version,
};
