use std::net::UdpSocket;

use crate::{torrent::Torrent, tracker::UriFmt};
mod torrent;
mod tracker;

// const PROTOCOL_ID: i64 = 0x41727101980;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // получаем и декодируем файл bencode
    let path = std::env::args().nth(1).expect("Не указан путь к файлу");
    let data = std::fs::read(path).expect("Не удалось прочитать файл");
    let t = Torrent::from_bytes(&data)?;

    /*
    Принято делать так: короткий префикс с названием клиента и версией,
    а остальное случайные байты.
    Например, -RS0001- (8 байт: «Rust», версия 0.0.0.1) плюс 12 случайных байт.
    Итого 8 + 12 = 20.
     */
    let mut peer_id = [0u8; 20];
    peer_id[..8].copy_from_slice(b"-RS0001-");
    peer_id[8..].copy_from_slice(&rand::random::<[u8; 12]>());

    if !t.files.is_empty() {
        println!("Файлы:");
        for f in &t.files {
            println!("  {} ({} байт)", f.path, f.length);
        }
    }
    println!("");

    println!("Трекер:       {}", t.announce);
    println!("Имя:          {}", t.name);
    println!("Инфо-хеш:     {}", t.info_hash_hex());
    println!("Кусков:       {}", t.pieces_count());

    println!(
        "Размер:       {:.2} МБ ({} байт)",
        t.total_length as f64 / 1_048_576.0,
        t.total_length
    );
    println!("Размер куска: {} КБ", t.piece_length / 1024);

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

    let connection_id = tracker::connect(&socket).unwrap();
    // let info_hash: [u8; 20] = Sha1::digest(info_raw).into();

    println!("connection id: {:?}", connection_id);

    let peers = tracker::announce(&socket, connection_id, &t, &peer_id).unwrap();

    println!("peers: {:?}", peers);

    let stream = 'peers: loop {
        for peer in peers {
            match tracker::handling_with_peers(&peer, &t, &peer_id) {
                Ok(stream) => break 'peers stream,
                Err(e) => {
                    println!("{peer}: {e}");
                    continue;
                }
            }
        }
        return Err("Не удалось подключиться ни к одному пиру".into());
    };

    print!("stream: {:?}", stream);
    Ok(())
}
