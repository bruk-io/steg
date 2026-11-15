//! Picking a new session's colour.

use steg_sdk::Colour;

/// Picks the colour a new session should take.
///
/// Scans the five colours in their fixed order (`Colour::ALL`) and returns
/// the first not already in `in_use` (R26: "a new session takes the next
/// colour not in use"). When every colour is in use, returns the first one
/// again (R109) rather than refusing or inventing a sixth.
pub(crate) fn next_colour(in_use: impl IntoIterator<Item = Colour>) -> Colour {
    let in_use: Vec<Colour> = in_use.into_iter().collect();
    Colour::ALL
        .into_iter()
        .find(|colour| !in_use.contains(colour))
        .unwrap_or(Colour::ALL[0])
}

#[cfg(test)]
mod tests {
    use super::next_colour;
    use steg_sdk::Colour;

    #[test]
    fn no_colours_in_use_gives_the_first() {
        assert_eq!(next_colour([]), Colour::Violet);
    }

    #[test]
    fn skips_colours_already_in_use() {
        assert_eq!(next_colour([Colour::Violet, Colour::Pink]), Colour::Orange);
    }

    #[test]
    fn wraps_to_the_first_colour_when_all_five_are_in_use() {
        assert_eq!(
            next_colour([
                Colour::Violet,
                Colour::Pink,
                Colour::Orange,
                Colour::Blue,
                Colour::Green,
            ]),
            Colour::Violet
        );
    }
}
