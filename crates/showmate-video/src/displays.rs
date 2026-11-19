/// Leest de schermen uit een output-naam (0-gebaseerd): `Display 1` → [0],
/// `Display 2` → [1], `Display 1+2` → [0, 1]. Onbekend valt terug op [0].
pub fn parse_displays(output: &str) -> Vec<usize> {
    let mut found: Vec<usize> = Vec::new();
    for n in output
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<usize>().ok())
    {
        if (1..=8).contains(&n) && !found.contains(&(n - 1)) {
            found.push(n - 1);
        }
    }
    if found.is_empty() { vec![0] } else { found }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names() {
        assert_eq!(parse_displays("Display 1"), vec![0]);
        assert_eq!(parse_displays("Display 2"), vec![1]);
        assert_eq!(parse_displays("Display 1+2"), vec![0, 1]);
        assert_eq!(parse_displays("Display 2+1"), vec![1, 0]);
        assert_eq!(parse_displays("Display 2+2"), vec![1]);
        assert_eq!(parse_displays(""), vec![0]);
        assert_eq!(parse_displays("Display 0"), vec![0]);
        assert_eq!(parse_displays("Display 99"), vec![0]);
    }
}
