use sha1::{Digest, Sha1};

mod bencode;

pub struct FileEntry {
    pub path: String,
    pub length: i64,
}

pub struct Torrent {
    pub announce: String,
    pub name: String,
    pub info_hash: [u8; 20],
    pub piece_length: i64,
    pub pieces: Vec<u8>, // склеенные SHA-1 всех кусков
    pub total_length: i64,
    pub files: Vec<FileEntry>, // пусто для торрента из одного файла
}

impl Torrent {
    pub fn from_bytes(data: &[u8]) -> Result<Torrent, Box<dyn std::error::Error>> {
        let root = bencode::decode(&data)?;
        let info_raw = bencode::info_bytes(&data)?;
        let info_hash: [u8; 20] = Sha1::digest(info_raw).into(); // 20 байт

        let announce = root
            .get("announce")
            .and_then(bencode::Value::as_str)
            .unwrap_or("-")
            .to_string();
        let info = root.get("info").ok_or("в файле нет секции info")?;

        let name = info
            .get("name")
            .and_then(bencode::Value::as_str)
            .unwrap_or("?")
            .to_string();

        let piece_length = info
            .get("piece length")
            .and_then(bencode::Value::as_int)
            .ok_or("нет piece length")?;

        let pieces = info
            .get("pieces")
            .and_then(bencode::Value::as_bytes)
            .ok_or("нет pieces")?
            .to_vec();

        let total_length: i64 =
            if let Some(len) = info.get("length").and_then(bencode::Value::as_int) {
                len
            } else {
                info.get("files")
                    .and_then(bencode::Value::as_list)
                    .map(|files| files.iter().filter_map(|f| f.get("length")?.as_int()).sum())
                    .ok_or("в торренте нет ни length, ни files")?
            };

        let mut files: Vec<FileEntry> = Vec::new();
        if let Some(list) = info.get("files").and_then(bencode::Value::as_list) {
            for f in list {
                let len = f
                    .get("length")
                    .and_then(bencode::Value::as_int)
                    .unwrap_or(0);
                let parts: Vec<&str> = f
                    .get("path")
                    .and_then(bencode::Value::as_list)
                    .map(|p| p.iter().filter_map(bencode::Value::as_str).collect())
                    .unwrap_or_default();

                files.push(FileEntry {
                    path: parts.join("/"),
                    length: len,
                });
            }
        }

        Ok(Torrent {
            announce,
            name,
            info_hash,
            piece_length,
            pieces,
            total_length,
            files,
        })
    }

    pub fn pieces_count(&self) -> usize {
        self.pieces.len() / 20
    }

    pub fn info_hash_hex(&self) -> String {
        self.info_hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}
