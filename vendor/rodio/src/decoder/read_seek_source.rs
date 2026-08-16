use std::io::{Read, Result, Seek, SeekFrom};

use symphonia::core::io::MediaSource;

pub struct ReadSeekSource<T: Read + Seek + Send + Sync> {
    inner: T,
    /// Totale lengte van de bron in bytes, bepaald bij het inpakken.
    ///
    /// Symphonia's format readers (FLAC, MP3, Ogg, WAV, …) hebben `byte_len()`
    /// nodig om accuraat te kunnen seaken; zonder dit faalt elke seek met
    /// `SeekErrorKind::Unseekable`.
    byte_len: Option<u64>,
}

impl<T: Read + Seek + Send + Sync> ReadSeekSource<T> {
    /// Instantiates a new `ReadSeekSource<T>` by taking ownership and wrapping the provided
    /// `Read + Seek`er.
    ///
    /// Bepaalt de totale lengte door eenmalig naar het einde van de bron te
    /// seeken en daarna terug te keren naar het begin.
    pub fn new(mut inner: T) -> Self {
        let byte_len = match inner.seek(SeekFrom::End(0)) {
            Ok(len) => {
                let _ = inner.seek(SeekFrom::Start(0));
                Some(len)
            }
            Err(_) => None,
        };
        ReadSeekSource { inner, byte_len }
    }
}

impl<T: Read + Seek + Send + Sync> MediaSource for ReadSeekSource<T> {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }
}

impl<T: Read + Seek + Send + Sync> Read for ReadSeekSource<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.read(buf)
    }
}

impl<T: Read + Seek + Send + Sync> Seek for ReadSeekSource<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.inner.seek(pos)
    }
}
