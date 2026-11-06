use sha1::{Digest, Sha1};

mod bencode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("Не указан путь к файлу");
    let data = std::fs::read(path).expect("Не удалось прочитать файл");
    let root = bencode::decode(&data).expect("Не удалось декодировать bencode");

    let info_raw = bencode::info_bytes(&data)?;
    let info_hash = Sha1::digest(info_raw); // 20 байт
    let info_hash_hex: String = info_hash.iter().map(|b| format!("{:02x}", b)).collect();

    let announce = root
        .get("announce")
        .and_then(bencode::Value::as_str)
        .unwrap_or("-");
    let info = root
        .get("info")
        .ok_or("в файле нет секции info")
        .expect("Ошибка при получении секции info");

    let name = info
        .get("name")
        .and_then(bencode::Value::as_str)
        .unwrap_or("?");

    let piece_length = info
        .get("piece length")
        .and_then(bencode::Value::as_int)
        .unwrap_or(0);
    let pieces_count = info
        .get("pieces")
        .and_then(bencode::Value::as_bytes)
        .map(|p| p.len() / 20) // каждый хеш SHA-1 это 20 байт
        .unwrap_or(0);

    let total: i64 = if let Some(len) = info.get("length").and_then(bencode::Value::as_int) {
        len
    } else {
        info.get("files")
            .and_then(bencode::Value::as_list)
            .map(|files| files.iter().filter_map(|f| f.get("length")?.as_int()).sum())
            .unwrap_or(0)
    };

    if let Some(files) = info.get("files").and_then(bencode::Value::as_list) {
        println!("Файлы:");
        for f in files {
            let len = f
                .get("length")
                .and_then(bencode::Value::as_int)
                .unwrap_or(0);
            let path: Vec<&str> = f
                .get("path")
                .and_then(bencode::Value::as_list)
                .map(|p| p.iter().filter_map(bencode::Value::as_str).collect())
                .unwrap_or_default();
            println!("  {} ({} байт)", path.join("/"), len);
        }
    }

    println!("Трекер:       {announce}");
    println!("Имя:          {name}");
    println!("Инфо-хеш:    {info_hash_hex}");
    println!(
        "Размер:       {:.2} МБ ({total} байт)",
        total as f64 / 1_048_576.0
    );
    println!("Размер куска: {} КБ", piece_length / 1024);
    println!("Кусков:       {pieces_count}");
    Ok(())
}
