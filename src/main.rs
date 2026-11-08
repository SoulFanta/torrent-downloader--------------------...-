use std::net::UdpSocket;

use crate::{peer::Peer, torrent::Torrent, tracker::UriFmt};
mod peer;
mod storage;
mod torrent;
mod tracker;

// const PROTOCOL_ID: i64 = 0x4 27101980;

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
    let Some(UriFmt::Udp {
        addr,
        path: tracker_path,
    }) = tracker::parse_uri(&uri)
    else {
        eprintln!("Неверный формат URI!");
        return Err("Неверный формат URI".into());
    };

    println!("IP: {}, Порт: {}", addr.ip(), addr.port());
    println!("Путь: /{}", tracker_path);

    // Биндимся на любой свободный локальный порт, а не на адрес трекера
    // (UdpSocket::bind обычно принимает локальный адрес вроде "0.0.0.0:0")
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    // Теперь переменная addr доступна, так как она объявлена в начале функции

    socket.connect(addr)?;

    let mut connection_id = tracker::connect(&socket)?;
    // let info_hash: [u8; 20] = Sha1::digest(info_raw).into();

    println!("connection id: {:?}", connection_id);

    let mut peers = tracker::announce(&socket, connection_id, &t, &peer_id)?;

    println!("peers count: {:?}", peers.len());

    let mut storage = storage::Storage::new("ubuntu.iso", &t)?;

    // ТЕСТ ПРОВЕРКА ЦЕЛОСТНОСТИ 10 КУСКОВ
    for index in 0..10 {
        let len = t.piece_len(index).ok_or("нет такого куска")?;
        let data = storage.read_piece(index, len)?;
        if !t.verify_piece(index, &data) {
            println!("кусок {index} на диске не совпадает");
        } else {
            println!("Совпало")
        }
    }

    let mut current: Option<Peer> = None;
    let mut next_addr = 0; // с какого адреса пробовать дальше
    let mut announces_left = 3;

    for index in 0..50u32 {
        if storage.already_have(&t, index).unwrap_or(false) {
            println!("кусок {index} уже есть");
            continue;
        }
        let start = std::time::Instant::now();

        let piece: Vec<u8> = loop {
            if let Some(peer) = current.as_mut() {
                if !peer.has_piece(index) {
                    println!("у пира нет куска {index}");
                    current = None;
                    continue;
                }
                match peer.download_piece(&t, index) {
                    Ok(piece) => break piece,
                    Err(e) => {
                        println!("пир отвалился: {e}");
                        current = None;
                    }
                }
            } else {
                if next_addr >= peers.len() {
                    if announces_left == 0 {
                        return Err("пиры закончились".into());
                    }
                    announces_left -= 1;
                    connection_id = tracker::connect(&socket)?;
                    peers = tracker::announce(&socket, connection_id, &t, &peer_id)?;
                    next_addr = 0;
                    if peers.is_empty() {
                        return Err("трекер не вернул ни одного пира".into());
                    }
                }
                let addr = &peers[next_addr];
                next_addr += 1;

                match Peer::open(addr, &t, &peer_id) {
                    Ok(p) => current = Some(p),
                    Err(e) => println!("{addr}: {e}"),
                }
            }
        };
        let secs = start.elapsed().as_secs_f64();
        let mb = t.piece_length as f64 / 1_048_576.0;
        storage.write_piece(index, &piece)?;
        println!("{:.1} МБ за {:.1} с = {:.2} МБ/с", mb, secs, mb / secs);
        println!("кусок {index}/{} скачан", t.pieces_count());
    }

    Ok(())
}
