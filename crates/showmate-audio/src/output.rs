/// Leest het uitgangspaar uit een output-naam: `Audio 1+2` → 0, `Audio 3+4` → 1.
/// Onbekende namen vallen terug op het eerste paar.
pub fn parse_output_pair(output: &str) -> usize {
    output
        .split(|c: char| !c.is_ascii_digit())
        .find_map(|n| n.parse::<usize>().ok())
        .map_or(0, |first| first.saturating_sub(1) / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs() {
        assert_eq!(parse_output_pair("Audio 1+2"), 0);
        assert_eq!(parse_output_pair("Audio 3+4"), 1);
        assert_eq!(parse_output_pair("Audio 5+6"), 2);
        assert_eq!(parse_output_pair("Display 1"), 0);
        assert_eq!(parse_output_pair(""), 0);
        assert_eq!(parse_output_pair("Audio 0"), 0);
    }
}
