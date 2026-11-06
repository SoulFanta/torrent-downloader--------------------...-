use std::{net::UdpSocket, time::Duration};

use crate::tracker::UriFmt;

mod tracker;

const PROTOCOL_ID: i64 = 0x41727101980;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uri = String::from("udp://tracker.torrent.eu.org:451/announce");

    // Изящный вариант с let else: если парсинг не удался, сразу возвращаем ошибку
    let Some(UriFmt::Udp { addr, path }) = tracker::parse_uri(&uri) else {
        eprintln!("Неверный формат URI!");
        return Err("Неверный формат URI".into());
    };

    println!("IP: {}, Порт: {}", addr.ip(), addr.port());
    println!("Путь: /{}", path);

    // Биндимся на любой свободный локальный порт, а не на адрес трекера
    // (UdpSocket::bind обычно принимает локальный адрес вроде "0.0.0.0:0")
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    // Теперь переменная addr доступна, так как она объявлена в начале функции
    socket.connect(addr)?;

    let action: i32 = 0;
    let transaction_id: i32 = rand::random();
    println!("transaction_id:  {}", transaction_id ); // [1, 2, 0, 0, 0, 1]

    let mut packet: Vec<u8> = Vec::new();

    packet.extend_from_slice(&PROTOCOL_ID.to_be_bytes());
    packet.extend_from_slice(&action.to_be_bytes());
    packet.extend_from_slice(&transaction_id.to_be_bytes());

    println!("{:?}", packet); // [1, 2, 0, 0, 0, 1]
    println!("{}", packet.len()); // 6

    socket
        .send(&packet)
        .expect("Не получилось отправить запрос");

    let mut buf: Vec<u8> = vec![0u8; 1024]; // буффер для ответа

    // Время ожидания ответа 5 секунд иначе ошибка
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    let n = socket.recv(&mut buf)?;
    let response = &buf[..n]; 


    println!("Получено байт: {}, данные: {:?}", n, response);
    let connection_id = i64::from_be_bytes(response[8..16].try_into().unwrap());
    println!("с 8 по 15 байт: {:?}", connection_id);

    Ok(())
}
