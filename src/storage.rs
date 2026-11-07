use std::{
    fs::{File, OpenOptions}, io::{self, Read, Seek, SeekFrom, Write},
};

use crate::torrent::Torrent;

pub struct Storage {
    file: File,
    piece_length: u64,
}

impl Storage {
    pub fn new(path: &str, t: &Torrent) -> io::Result<Storage> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;

        let size = u64::try_from(t.total_length)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "отрицательный размер"))?;
        let piece_length = u64::try_from(t.piece_length)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "отрицательный размер"))?;

        file.set_len(size)?;
        Ok(Storage { file, piece_length })
    }
    pub fn write_piece(&mut self, index: u32, data: &[u8]) -> io::Result<()> {
        self.file
            .seek(SeekFrom::Start(index as u64 * self.piece_length))?;
        self.file.write_all(data)
    }

    pub fn read_piece(&mut self, index: u32, len: usize) -> io::Result<Vec<u8>> {
        self.file
            .seek(SeekFrom::Start(index as u64 * self.piece_length))?;

        let mut buf = vec![0u8; len];
        self.file.read_exact(&mut buf)?;

        Ok(buf)
    }
}
