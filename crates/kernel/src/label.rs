//! Validating a label (doc-15 R10; doc-2 section 4).
//!
//! A label may not contain `/` — the path separator a target is split on —
//! and may not be made only of digits, so a path always reads one way: a
//! part that is all digits is a position, never a label. Every setter
//! (`Session::set_label`, `Tab::set_label`, `Pane::set_label`) and every
//! place a label is given at creation (`Tree::new_session`, `Tree::new_tab`)
//! calls this one function, so the rule lives in exactly one place.
//!
//! These two bans are also what keeps `target.rs`'s match order coherent: a
//! numeric part can never be a label, so it is always free to match as a
//! position (R10's third kind) once no identifier or label has claimed it.

use crate::error::Error;

/// Validates a label before it is stored, passing `None` through unchanged.
///
/// # Errors
///
/// Returns [`Error::InvalidLabel`] if the label contains `/`, or is made
/// only of digits (an empty string has no digits to be "only digits", and
/// is left as a separate, undecided question — this function does not
/// refuse it).
pub(crate) fn validate(label: Option<String>) -> Result<Option<String>, Error> {
    match label {
        Some(label) => {
            let is_all_digits = !label.is_empty() && label.chars().all(|c| c.is_ascii_digit());
            if label.contains('/') || is_all_digits {
                Err(Error::InvalidLabel(label))
            } else {
                Ok(Some(label))
            }
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::error::Error;

    #[test]
    fn rejects_a_label_containing_a_slash() {
        assert_eq!(
            validate(Some("work/logs".to_string())),
            Err(Error::InvalidLabel("work/logs".to_string()))
        );
    }

    #[test]
    fn rejects_a_label_made_only_of_digits() {
        assert_eq!(
            validate(Some("123".to_string())),
            Err(Error::InvalidLabel("123".to_string()))
        );
    }

    #[test]
    fn accepts_an_ordinary_label() {
        assert_eq!(
            validate(Some("logs".to_string())),
            Ok(Some("logs".to_string()))
        );
    }

    #[test]
    fn accepts_a_label_that_mixes_digits_and_letters() {
        assert_eq!(
            validate(Some("logs2".to_string())),
            Ok(Some("logs2".to_string()))
        );
    }

    #[test]
    fn passes_a_cleared_label_through_unchanged() {
        assert_eq!(validate(None), Ok(None));
    }
}
