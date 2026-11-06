/*
    Вам нужно отправить трекеру набор байтов определённой раскладки и разобрать набор байтов в ответ.
    Вся сложность в том, что раскладка фиксирована.
    Каждое поле лежит на своём месте и занимает свой размер.
    Представьте таблицу, в которой строки идут подряд без разделителей.
*/

/*

    Два факта, на которых всё держится:

    Числа передаются big-endian, то есть старший байт первым. Например, 1 как i32 это байты 00 00 00 01.
    В Rust для этого есть to_be_bytes() (число в массив байт) и from_be_bytes() (обратно).
    UDP это просто «бросил пакет, жди пакет в ответ». Нет установления соединения, поэтому у вас будет send и recv.

*/

// Шаг 1

// Connect

use crate::torrent::Torrent;
use std::io::{Read, Write};
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, ToSocketAddrs, UdpSocket},
    time::Duration,
};

const PROTOCOL_ID: i64 = 0x41727101980;
// tcp
const MAX_SIZE: usize = 1024 * 1024;

#[derive(Debug)]
pub enum UriFmt {
    Udp { addr: SocketAddr, path: String },
}

pub fn parse_uri(uri: &str) -> Option<UriFmt> {
    // strip_prefix работает с &str, возвращая срез той же строки
    let stripped = uri.strip_prefix("udp://")?;

    let mut parts = stripped.splitn(2, '/');
    let addr_str = parts.next()?;
    let path = parts.next().unwrap_or("").to_string();

    let addr = addr_str.to_socket_addrs().ok()?.next()?;

    Some(UriFmt::Udp { addr, path })
}

pub fn connect(socket: &UdpSocket) -> Result<i64, std::io::Error> {
    let action: i32 = 0;
    let transaction_id: u32 = rand::random();

    let mut packet: Vec<u8> = Vec::new();
    packet.extend_from_slice(&PROTOCOL_ID.to_be_bytes());
    packet.extend_from_slice(&action.to_be_bytes());
    packet.extend_from_slice(&transaction_id.to_be_bytes());

    socket
        .send(&packet)
        .expect("Не получилось отправить запрос");

    let mut buf: Vec<u8> = vec![0u8; 4096]; // буффер для ответа
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let n = socket.recv(&mut buf)?;
    let response = &buf[..n];

    // Проверка транзакции на верность
    if transaction_id != u32::from_be_bytes(response[4..8].try_into().unwrap()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Tracker response transaction ID does not match",
        ));
    }

    let connection_id = i64::from_be_bytes(response[8..16].try_into().unwrap());

    Ok(connection_id)
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(buf[offset..offset + 4].try_into().unwrap())
}

pub fn announce(
    socket: &UdpSocket,
    connection_id: i64,
    torrent: &Torrent,
    peer_id: &[u8; 20],
) -> Result<Vec<SocketAddrV4>, Box<dyn std::error::Error>> {
    let mut peers: Vec<SocketAddrV4> = Vec::new();

    let transaction_id: u32 = rand::random();
    let key: u32 = rand::random();
    let port: u16 = 6881;
    let left = torrent.total_length;

    let mut packet: Vec<u8> = Vec::new();
    packet.extend_from_slice(&connection_id.to_be_bytes());
    packet.extend_from_slice(&1i32.to_be_bytes()); // action
    packet.extend_from_slice(&transaction_id.to_be_bytes());
    packet.extend_from_slice(&torrent.info_hash);
    packet.extend_from_slice(peer_id);
    packet.extend_from_slice(&0i64.to_be_bytes()); // downloaded
    packet.extend_from_slice(&left.to_be_bytes()); // left
    packet.extend_from_slice(&0i64.to_be_bytes()); // upload
    packet.extend_from_slice(&2i32.to_be_bytes()); // event
    packet.extend_from_slice(&0i32.to_be_bytes()); // ip
    packet.extend_from_slice(&key.to_be_bytes()); // key
    packet.extend_from_slice(&(-1i32).to_be_bytes()); // num_want
    packet.extend_from_slice(&port.to_be_bytes()); // port

    socket.send(&packet)?;
    let mut buf = vec![0u8; 4096];
    let n = socket.recv(&mut buf)?;
    let response = &buf[..n];

    if n < 8 {
        return Err("слишком короткий ответ".into());
    }
    //  i64::from_be_bytes(response[8..16].try_into().unwrap());
    let action: u32 = u32::from_be_bytes(response[0..4].try_into().unwrap());
    if action == 3 {
        eprint!("Error: {}", String::from_utf8_lossy(&response[8..]));
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&response[8..]).into_owned(),
        )
        .into());
    }

    if action != 1 || n < 20 {
        return Err("Какая то мутная тема".into());
    }

    let action = read_u32(response, 0);
    let resp_tx = read_u32(response, 4);
    let interval = read_u32(response, 8);
    let leechers = read_u32(response, 12);
    let seeders = read_u32(response, 16);

    if resp_tx != transaction_id {
        return Err(
            std::io::Error::new(std::io::ErrorKind::Other, "No match transaction id").into(),
        );
    }

    println!("action: {}", action);
    println!("resp_tx: {}", resp_tx);
    println!("interval: {}", interval);
    println!("seeders: {}", seeders);
    println!("leechers: {}", leechers);

    for chunk in response[20..].chunks_exact(6) {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        peers.push(SocketAddrV4::new(ip, port));
    }

    Ok(peers)
}

pub fn handling_with_peers(
    peer: &SocketAddrV4,
    torrent: &Torrent,
    peer_id: &[u8; 20],
) -> Result<TcpStream, Box<dyn std::error::Error>> {
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

    Ok(stream)
}

pub fn read_message(
    stream: &mut TcpStream,
) -> Result<Option<(u8, Vec<u8>)>, Box<dyn std::error::Error>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let length = u32::from_be_bytes(len_buf) as usize;

    if length == 0 {
        return Ok(None); // keep-alive
    }
    if length > MAX_SIZE {
        return Err("Превышен лимит в 1 МБ".into());
    }

    let mut body = vec![0u8; length];
    stream.read_exact(&mut body)?;

    let id = body[0];
    let payload = body[1..].to_vec();
    Ok(Some((id, payload)))
}
