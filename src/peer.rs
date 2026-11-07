use sha1::{Digest, Sha1};
use std::{
    io::{Read, Write},
    net::{SocketAddrV4, TcpStream},
};

use crate::torrent::Torrent;

pub const MAX_SIZE: usize = 1024 * 1024;

/*

    только TCP: рукопожатие, сообщения, блоки, куски

*/

// id	Название	Смысл
// 0	choke	пир не будет отдавать вам данные
// 1	unchoke	пир разрешил брать у него данные
// 2	interested	«я хочу у тебя что-то скачать» (это посылаете вы)
// 5	bitfield	карта кусков, которые есть у пира
// 6	request	«пришли мне такой-то блок»
// 7	piece	сам блок данных

pub struct Peer {
    stream: TcpStream,
    bitfield: Vec<u8>, // карта кусков пира
}

impl Peer {
    pub fn connect(
        peer: &SocketAddrV4,
        torrent: &Torrent,
        peer_id: &[u8; 20],
    ) -> Result<Peer, Box<dyn std::error::Error>> {
        let mut stream = TcpStream::connect_timeout(
            &std::net::SocketAddr::V4(*peer),
            std::time::Duration::from_secs(3),
        )?;

        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;

        let mut handshake: Vec<u8> = Vec::new();
        handshake.push(19);
        handshake.extend_from_slice(b"BitTorrent protocol");
        handshake.extend_from_slice(&[0u8; 8]); // 8 нулей
        handshake.extend_from_slice(&torrent.info_hash);
        handshake.extend_from_slice(peer_id);

        let mut reply = [0u8; 68];

        stream.write_all(&handshake)?;
        stream.read_exact(&mut reply)?;

        if reply[0] != 19 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid BitTorrent handshake",
            )
            .into());
        }
        if &reply[1..20] != b"BitTorrent protocol" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid BitTorrent handshake",
            )
            .into());
        }
        if &reply[28..48] != torrent.info_hash {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid BitTorrent handshake",
            )
            .into());
        }

        Ok(Peer {
            stream,
            bitfield: Vec::new(),
        })
    }

    // отправляет interested и ждёт id == 1, unchoke
    pub fn wait_unchoke(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stream.write_all(&[0, 0, 0, 1, 2])?;

        for _ in 0..50 {
            match self.read_message()? {
                None => {}
                Some((1, _)) => return Ok(()),
                Some((5, payload)) => self.bitfield = payload, // сохраняем карту кусков
                Some((_, _)) => {}
            }
        }

        Err("Ни один пир не unchoke".into())
    }

    pub fn read_message(&mut self) -> Result<Option<(u8, Vec<u8>)>, Box<dyn std::error::Error>> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let length = u32::from_be_bytes(len_buf) as usize;

        if length == 0 {
            return Ok(None); // keep-alive
        }
        if length > MAX_SIZE {
            return Err("Превышен лимит в 1 МБ".into());
        }

        let mut body = vec![0u8; length];
        self.stream.read_exact(&mut body)?;

        let id = body[0];
        let payload = body[1..].to_vec();
        Ok(Some((id, payload)))
    }

    pub fn request_block(
        &mut self,
        index: u32,
        begin: u32,
        length: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut request: Vec<u8> = Vec::new();
        request.extend_from_slice(&13i32.to_be_bytes());
        request.push(6);
        request.extend_from_slice(&index.to_be_bytes()); // index
        request.extend_from_slice(&begin.to_be_bytes()); // begin
        request.extend_from_slice(&length.to_be_bytes()); // length

        match self.stream.write_all(&request) {
            Ok(()) => {
                return Ok(());
            }
            Err(err) => {
                return Err(format!("Ошибка при отправке: {}", err).into());
            }
        };
    }

    pub fn read_block(
        &mut self,
        index: u32,
        begin: u32,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        for _ in 0..50 {
            match self.read_message()? {
                None => continue, // keep-alive
                Some((7, payload)) => {
                    if payload.len() < 8 {
                        return Err("слишком короткий piece".into());
                    }
                    let got_index = u32::from_be_bytes(payload[0..4].try_into().unwrap());
                    let got_begin = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                    if got_index != index || got_begin != begin {
                        return Err("пир прислал не тот блок".into());
                    }
                    return Ok(payload[8..].to_vec());
                }
                Some((id, _)) => println!("пропускаю сообщение id={id}"),
            }
        }
        Err("не дождались блока".into())
    }

    pub fn download_piece(
        &mut self,
        t: &Torrent,
        index: u32,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let piece_len = t.piece_len(index).ok_or("нет такого куска")?;
        if piece_len > 16 * (1024 * 1024) {
            return Err("слишком длинная часть".into());
        }

        let mut buffer = vec![0u8; piece_len];

        for begin in (0..piece_len).step_by(16384) {
            let block_len = std::cmp::min(16384, piece_len - begin);

            self.request_block(index, begin as u32, block_len as u32)?;
            let data = self.read_block(index, begin as u32)?;
            if data.len() != block_len {
                return Err("Длинна блоков не совпадает".into());
            }
            buffer[begin..begin + block_len].copy_from_slice(&data);
        }

        // проверка хэша
        let got: [u8; 20] = Sha1::digest(&buffer).into();

        let a = index as usize * 20;
        let expected = &t.pieces[a..a + 20];

        if got[..] != *expected {
            return Err("хэш куска не совпал".into());
        }

        Ok(buffer)
    }

    #[warn(dead_code)]
    pub fn connect_any(
        peers: &[SocketAddrV4],
        t: &Torrent,
        peer_id: &[u8; 20],
    ) -> Result<Peer, Box<dyn std::error::Error>> {
        loop {
            for peer in peers {
                match Peer::connect(&peer, &t, &peer_id) {
                    Ok(peer) => {
                        return Ok(peer);
                    }
                    Err(e) => {
                        println!("{peer}: {e}");
                        continue;
                    }
                }
            }
            return Err("Не удалось подключиться ни к одному пиру".into());
        }
    }

    pub fn open(
        addr: &SocketAddrV4,
        t: &Torrent,
        peer_id: &[u8; 20],
    ) -> Result<Peer, Box<dyn std::error::Error>> {
        let mut peer = Peer::connect(addr, t, peer_id)?;
        peer.wait_unchoke()?;
        Ok(peer)
    }
}

pub fn try_peer(
    addr: &SocketAddrV4,
    t: &Torrent,
    peer_id: &[u8; 20],
    index: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut peer = Peer::connect(addr, t, peer_id)?;
    peer.wait_unchoke()?;
    peer.download_piece(t, index)
}

pub fn download_from_any(
    peers: &[SocketAddrV4],
    t: &Torrent,
    peer_id: &[u8; 20],
    index: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    for addr in peers {
        match try_peer(addr, t, peer_id, index) {
            Ok(piece) => return Ok(piece),
            Err(e) => println!("{addr}: {e}"),
        }
    }
    Err("ни один пир не отдал кусок".into())
}
