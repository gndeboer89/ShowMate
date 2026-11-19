#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("bestand niet te lezen: {0}")]
    Io(#[from] std::io::Error),
    #[error("kan audio niet decoderen: {0}")]
    Decode(String),
    #[error("geen audiospoor in bestand")]
    NoTrack,
    #[error("audio-apparaat: {0}")]
    Device(String),
}
