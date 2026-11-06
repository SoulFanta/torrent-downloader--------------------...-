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

use std::net::{SocketAddr, ToSocketAddrs};

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