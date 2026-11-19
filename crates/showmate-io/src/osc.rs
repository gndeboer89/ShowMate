use rosc::{OscMessage, OscPacket, OscType, encoder};
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// Leest argumenten uit tekst. Gescheiden door spaties; `"…"` voor tekst met spaties.
///
/// Type wordt geraden (`5` → int, `0.7` → float, `true`/`false` → bool, rest → tekst) of
/// expliciet gekozen met een prefix: `i:`, `f:`, `s:`, `b:`. Veel apparaten (o.a. lichttafels)
/// eisen floats; schrijf dan `f:0` in plaats van `0`.
pub fn parse_args(text: &str) -> Result<Vec<OscType>, String> {
    let mut args = Vec::new();
    for tok in tokenize(text)? {
        args.push(parse_token(&tok)?);
    }
    Ok(args)
}

struct Token {
    text: String,
    quoted: bool,
}

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some(e) => s.push(e),
                        None => return Err("tekst eindigt op '\\'".into()),
                    },
                    Some(ch) => s.push(ch),
                    None => return Err("aanhalingsteken niet afgesloten".into()),
                }
            }
            out.push(Token {
                text: s,
                quoted: true,
            });
        } else {
            let mut s = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                s.push(ch);
                chars.next();
            }
            out.push(Token {
                text: s,
                quoted: false,
            });
        }
    }
    Ok(out)
}

fn parse_token(t: &Token) -> Result<OscType, String> {
    if t.quoted {
        return Ok(OscType::String(t.text.clone()));
    }
    let s = t.text.as_str();
    let bad = |what: &str| format!("'{s}' is geen geldige {what}");
    if let Some(v) = s.strip_prefix("i:") {
        return v.parse().map(OscType::Int).map_err(|_| bad("integer"));
    }
    if let Some(v) = s.strip_prefix("f:") {
        return v
            .replace(',', ".")
            .parse()
            .map(OscType::Float)
            .map_err(|_| bad("float"));
    }
    if let Some(v) = s.strip_prefix("b:") {
        return match v {
            "true" | "1" => Ok(OscType::Bool(true)),
            "false" | "0" => Ok(OscType::Bool(false)),
            _ => Err(bad("bool")),
        };
    }
    if let Some(v) = s.strip_prefix("s:") {
        return Ok(OscType::String(v.to_string()));
    }
    Ok(match s {
        "true" => OscType::Bool(true),
        "false" => OscType::Bool(false),
        _ => {
            if let Ok(i) = s.parse::<i32>() {
                OscType::Int(i)
            } else if let Ok(f) = s.replace(',', ".").parse::<f32>() {
                OscType::Float(f)
            } else {
                OscType::String(s.to_string())
            }
        }
    })
}

/// Tekstweergave van argumenten, zodat ze weer te parsen zijn (voor de lijst in de UI).
pub fn format_args(args: &[OscType]) -> String {
    args.iter()
        .map(|a| match a {
            OscType::Int(i) => i.to_string(),
            OscType::Float(f) => format!("f:{f}"),
            OscType::Bool(b) => b.to_string(),
            OscType::String(s) => format!("\"{s}\""),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Codeert een OSC-bericht. Het adres moet met `/` beginnen.
pub fn encode_message(address: &str, args: Vec<OscType>) -> Result<Vec<u8>, String> {
    let address = address.trim();
    if !address.starts_with('/') {
        return Err("OSC-adres moet met '/' beginnen".into());
    }
    if address.chars().any(char::is_whitespace) {
        return Err("OSC-adres mag geen spaties bevatten".into());
    }
    encoder::encode(&OscPacket::Message(OscMessage {
        addr: address.to_string(),
        args,
    }))
    .map_err(|e| format!("kan OSC niet coderen: {e}"))
}

/// Verstuurt OSC over UDP. Adressen worden per host:poort onthouden zodat DNS niet bij elke
/// verzending wacht.
#[derive(Default)]
pub struct OscSender {
    v4: Option<UdpSocket>,
    v6: Option<UdpSocket>,
    resolved: HashMap<(String, u16), SocketAddr>,
}

impl OscSender {
    pub fn new() -> Self {
        Self::default()
    }

    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddr, String> {
        let key = (host.trim().to_string(), port);
        if let Some(addr) = self.resolved.get(&key) {
            return Ok(*addr);
        }
        let addr = (key.0.as_str(), port)
            .to_socket_addrs()
            .map_err(|e| format!("kan '{host}' niet vinden: {e}"))?
            .next()
            .ok_or_else(|| format!("geen adres voor '{host}'"))?;
        self.resolved.insert(key, addr);
        Ok(addr)
    }

    /// Vergeet gecachete adressen (na een wijziging in de instellingen).
    pub fn forget(&mut self) {
        self.resolved.clear();
    }

    fn socket(&mut self, addr: &SocketAddr) -> Result<&UdpSocket, String> {
        let (slot, bind) = if addr.is_ipv4() {
            (&mut self.v4, "0.0.0.0:0")
        } else {
            (&mut self.v6, "[::]:0")
        };
        if slot.is_none() {
            *slot = Some(
                UdpSocket::bind(bind).map_err(|e| format!("kan UDP-socket niet openen: {e}"))?,
            );
        }
        Ok(slot.as_ref().expect("zojuist aangemaakt"))
    }

    pub fn send(
        &mut self,
        host: &str,
        port: u16,
        address: &str,
        args: Vec<OscType>,
    ) -> Result<(), String> {
        let bytes = encode_message(address, args)?;
        let addr = self.resolve(host, port)?;
        self.socket(&addr)?
            .send_to(&bytes, addr)
            .map_err(|e| format!("verzenden naar {addr} mislukt: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guesses_and_forces_types() {
        assert_eq!(
            parse_args(r#"5 0.7 true "hallo wereld" f:0 i:3 s:abc b:false woord"#).unwrap(),
            vec![
                OscType::Int(5),
                OscType::Float(0.7),
                OscType::Bool(true),
                OscType::String("hallo wereld".into()),
                OscType::Float(0.0),
                OscType::Int(3),
                OscType::String("abc".into()),
                OscType::Bool(false),
                OscType::String("woord".into()),
            ]
        );
        assert_eq!(parse_args("").unwrap(), vec![]);
        assert_eq!(parse_args("  ").unwrap(), vec![]);
        assert_eq!(parse_args("0,5").unwrap(), vec![OscType::Float(0.5)]);
        assert_eq!(
            parse_args(r#""a \"b\"""#).unwrap(),
            vec![OscType::String("a \"b\"".into())]
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_args("i:abc").is_err());
        assert!(parse_args("f:").is_err());
        assert!(parse_args("b:misschien").is_err());
        assert!(parse_args("\"open").is_err());
    }

    #[test]
    fn validates_address() {
        assert!(encode_message("/a/b", vec![]).is_ok());
        assert!(encode_message("a/b", vec![]).is_err());
        assert!(encode_message("/a b", vec![]).is_err());
        assert!(encode_message("", vec![]).is_err());
    }

    #[test]
    fn formats_round_trip() {
        let args = parse_args(r#"1 f:0.5 true "x y""#).unwrap();
        assert_eq!(parse_args(&format_args(&args)).unwrap(), args);
    }
}
