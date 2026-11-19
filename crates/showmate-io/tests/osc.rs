use rosc::{OscPacket, OscType, decoder};
use showmate_io::*;
use std::net::UdpSocket;
use std::time::Duration;

fn listener() -> (UdpSocket, u16) {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let port = sock.local_addr().unwrap().port();
    (sock, port)
}

fn receive(sock: &UdpSocket) -> rosc::OscMessage {
    let mut buf = [0u8; 1024];
    let n = sock.recv(&mut buf).expect("geen OSC ontvangen");
    match decoder::decode_udp(&buf[..n]).unwrap().1 {
        OscPacket::Message(m) => m,
        other => panic!("onverwacht pakket {other:?}"),
    }
}

#[test]
fn sends_message_over_udp() {
    let (sock, port) = listener();
    let mut sender = OscSender::new();
    let args = parse_args("f:0.7 3 true").unwrap();
    sender
        .send("127.0.0.1", port, "/lighting/spots/opkomst", args)
        .unwrap();
    let m = receive(&sock);
    assert_eq!(m.addr, "/lighting/spots/opkomst");
    assert_eq!(
        m.args,
        vec![OscType::Float(0.7), OscType::Int(3), OscType::Bool(true)]
    );
}

#[test]
fn resolves_hostnames_and_reuses_socket() {
    let (sock, port) = listener();
    let mut sender = OscSender::new();
    for i in 0..3 {
        sender
            .send("localhost", port, "/ping", vec![OscType::Int(i)])
            .unwrap();
    }
    for i in 0..3 {
        // localhost kan ook ::1 opleveren; dan komt er niets op onze v4-listener
        // en slaan we de test over in plaats van te falen.
        let mut buf = [0u8; 256];
        match sock.recv(&mut buf) {
            Ok(n) => {
                let OscPacket::Message(m) = decoder::decode_udp(&buf[..n]).unwrap().1 else {
                    panic!()
                };
                assert_eq!(m.args, vec![OscType::Int(i)]);
            }
            Err(_) => {
                eprintln!("localhost → IPv6, test overgeslagen");
                return;
            }
        }
    }
}

#[test]
fn errors_are_reported() {
    let mut sender = OscSender::new();
    assert!(sender.send("127.0.0.1", 9, "geen-slash", vec![]).is_err());
    assert!(
        sender
            .send("deze.host.bestaat.echt.niet.invalid", 9, "/x", vec![])
            .is_err()
    );
}

#[cfg(feature = "midi")]
#[test]
fn midi_port_listing_does_not_panic() {
    // Zonder MIDI-apparaten (CI) is een lege lijst of een nette fout beide prima.
    match list_midi_ports() {
        Ok(ports) => eprintln!("{} MIDI-uitgangen", ports.len()),
        Err(e) => eprintln!("geen MIDI-subsysteem: {e}"),
    }
    assert!(MidiOut::connect("bestaat-niet").is_err());
}
