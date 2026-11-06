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

use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    time::Duration,
};

const PROTOCOL_ID: i64 = 0x41727101980;

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

pub fn announce(socket: &UdpSocket, connection_id: i64, info_hash: &[u8; 20]) {
    let transaction_id: u32 = rand::random();

    let mut packet: Vec<u8> = Vec::new();
    packet.extend_from_slice(&connection_id.to_be_bytes());
    packet.extend_from_slice(&1i32.to_be_bytes());
    packet.extend_from_slice(&transaction_id.to_be_bytes());
    packet.extend_from_slice(&transaction_id.to_be_bytes());
}
