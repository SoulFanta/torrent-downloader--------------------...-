use std::collections::BTreeMap;

// Bencode
// i42e — целое число
// 4:spam — байтовая строка (длина, двоеточие, данные)
// l...e — список
// d...e — словарь (ключи — байтовые строки)

// |_| не важно какая ошибка внутри

// это символ который указывает на конец числа, списка, словаря или байтовой строки
// terminator character: 'e' for int, list, dict; ':' for bytes

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Dict(BTreeMap<Vec<u8>, Value>),
}

#[derive(Debug)]
pub enum Error {
    UnexpectedEof,
    InvalidByte(u8, usize), // байт, позиция
    InvalidNumber(usize),
    TrailingData(usize),
    MissingInfo,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::UnexpectedEof => write!(f, "неожиданный конец данных"),
            Error::InvalidByte(b, pos) => write!(f, "неверный байт {:#x} на позиции {}", b, pos),
            Error::InvalidNumber(pos) => write!(f, "неверное число на позиции {}", pos),
            Error::TrailingData(pos) => write!(f, "лишние данные после позиции {}", pos),
            Error::MissingInfo => write!(f, "отсутствует информация о торрентах"),
        }
    }
}



struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    // берем текущий байт, не продвигая позицию
    fn peek(&self) -> Result<u8, Error> {
        self.data.get(self.pos).copied().ok_or(Error::UnexpectedEof)
    }

    fn parse(&mut self) -> Result<Value, Error> {
        match self.peek()? {
            b'i' => self.parse_int(),
            b'l' => self.parse_list(),
            b'd' => self.parse_dict(),
            b'0'..=b'9' => Ok(Value::Bytes(self.parse_bytes()?)),
            _ => Err(Error::InvalidByte(self.peek().unwrap_or(0), self.pos)),
        }
    }

    fn read_number(&mut self, terminator: u8) -> Result<i64, Error> {
        let start = self.pos;
        while self.peek()? != terminator {
            self.pos += 1;
        }
        let s =
            str::from_utf8(&self.data[start..self.pos]).map_err(|_| Error::InvalidNumber(start))?;

        self.pos += 1; // пропускаем терминатор
        s.parse().map_err(|_| Error::InvalidNumber(start))
    }

    fn parse_int(&mut self) -> Result<Value, Error> {
        self.pos += 1; // 'i' пропускаем
        Ok(Value::Int(self.read_number(b'e')?))
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let start = self.pos;
        let len =
            usize::try_from(self.read_number(b':')?).map_err(|_| Error::InvalidNumber(start))?;

        let end = self.pos.checked_add(len).ok_or(Error::UnexpectedEof)?;
        let bytes = self.data.get(self.pos..end).ok_or(Error::UnexpectedEof)?;
        self.pos = end;
        Ok(bytes.to_vec())
    }

    fn parse_list(&mut self) -> Result<Value, Error> {
        self.pos += 1; // 'l' пропускаем
        let mut items = Vec::new();
        while self.peek()? != b'e' {
            items.push(self.parse()?);
        }
        self.pos += 1; // 'e' пропускаем
        Ok(Value::List(items))
    }

    fn parse_dict(&mut self) -> Result<Value, Error> {
        self.pos += 1; // 'd' пропускаем
        let mut map = BTreeMap::new();
        while self.peek()? != b'e' {
            if !self.peek()?.is_ascii_digit() {
                return Err(Error::InvalidByte(self.peek().unwrap_or(0), self.pos));
            }
            let key = self.parse_bytes()?;
            let value = self.parse()?;
            map.insert(key, value);
        }
        self.pos += 1; // 'e' пропускаем
        Ok(Value::Dict(map))
    }
}

pub fn decode(data: &[u8]) -> Result<Value, Error> {
    let mut p = Parser { data, pos: 0 };
    let value = p.parse()?;
    if p.pos != data.len() {
        return Err(Error::TrailingData(p.pos));
    }
    Ok(value)
}

 pub fn info_bytes(data: &[u8]) -> Result<&[u8], Error> {
    let mut p = Parser { data, pos: 0 };

    let first = p.peek()?;

    // проверяем что это словарь, иначе не будет секции info
    if first != b'd' {
        return Err(Error::InvalidByte(first, 0));
    }
    p.pos += 1; // 'd' пропускаем

    loop {
        match p.peek()? {
            // если достигли конца словаря, значит info не найдена
            b'e' => return Err(Error::MissingInfo),
            // ключи словаря — это байтовые строки, начинающиеся с цифры
            b'0'..=b'9' => {
                let key = p.parse_bytes()?;
                let start = p.pos;  // значение начинается здесь
                p.parse()?;         // разобрали и пропустили его
                if key == b"info" {
                    return Ok(&data[start..p.pos]);
                }
            }
            // если встретили что-то неожиданное, возвращаем ошибку
            byte => return Err(Error::InvalidByte(byte, p.pos)),
        }
    }
}



impl Value {
    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(i) = self {
            Some(*i)
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        if let Value::Bytes(b) = self {
            Some(b)
        } else {
            None
        }
    }

    /// Строка, если байты валидный UTF-8
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_bytes()?).ok()
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        if let Value::List(l) = self {
            Some(l)
        } else {
            None
        }
    }

    pub fn as_dict(&self) -> Option<&BTreeMap<Vec<u8>, Value>> {
        if let Value::Dict(d) = self {
            Some(d)
        } else {
            None
        }
    }

    /// Достаёт значение по ключу, если это словарь
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_dict()?.get(key.as_bytes())
    }
}
