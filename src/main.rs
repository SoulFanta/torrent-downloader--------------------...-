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

    let connection_id = tracker::connect(&socket);
    // let info_hash: [u8; 20] = Sha1::digest(info_raw).into();

    println!("connection id: {:?}", connection_id.unwrap());

    // Время ожидания ответа 5 секунд иначе ошибка

    Ok(())
}
