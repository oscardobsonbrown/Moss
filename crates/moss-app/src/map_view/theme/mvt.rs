use std::collections::BTreeMap;

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_FEATURES: usize = 30_000;
const MAX_WORDS: usize = 800_000;

pub struct Layer {
    pub name: String,
    pub extent: u32,
    pub features: Vec<Feature>,
}
pub struct Feature {
    pub kind: u8,
    pub properties: BTreeMap<String, String>,
    pub geometry: Vec<u32>,
}
pub struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn done(&self) -> bool {
        self.at == self.bytes.len()
    }
    pub fn number(&mut self) -> Result<u64, String> {
        let mut value = 0;
        for i in 0..10 {
            let byte = *self.bytes.get(self.at).ok_or("Truncated vector tile.")?;
            self.at += 1;
            if i == 9 && byte > 1 {
                return Err("Invalid vector tile integer.".into());
            }
            value |= u64::from(byte & 127) << (i * 7);
            if byte < 128 {
                return Ok(value);
            }
        }
        Err("Invalid vector tile integer.".into())
    }
    fn take(&mut self, size: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(size)
            .ok_or("Vector tile length overflow.")?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or("Truncated vector tile.")?;
        self.at = end;
        Ok(bytes)
    }
    fn data(&mut self) -> Result<&'a [u8], String> {
        let size = usize::try_from(self.number()?).map_err(|_| "Vector tile length overflow.")?;
        self.take(size)
    }
    fn field(&mut self) -> Result<(u64, u8), String> {
        let key = self.number()?;
        if key >> 3 == 0 {
            return Err("Invalid vector tile field.".into());
        }
        Ok((key >> 3, (key & 7) as u8))
    }
    fn skip(&mut self, wire: u8) -> Result<(), String> {
        match wire {
            0 => {
                self.number()?;
            }
            1 => {
                self.take(8)?;
            }
            2 => {
                self.data()?;
            }
            5 => {
                self.take(4)?;
            }
            _ => return Err("Unsupported vector tile field.".into()),
        }
        Ok(())
    }
}
fn string(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > 4096 {
        return Err("Vector tile text is too long.".into());
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| "Invalid vector tile text.".into())
}
fn value(bytes: &[u8]) -> Result<String, String> {
    let mut cursor = Cursor::new(bytes);
    let mut value = String::new();
    while !cursor.done() {
        let (field, wire) = cursor.field()?;
        value = match (field, wire) {
            (1, 2) => string(cursor.data()?)?,
            (2, 5) => f32::from_le_bytes(cursor.take(4)?.try_into().unwrap()).to_string(),
            (3, 1) => f64::from_le_bytes(cursor.take(8)?.try_into().unwrap()).to_string(),
            (4 | 5, 0) => cursor.number()?.to_string(),
            (6, 0) => {
                let n = cursor.number()?;
                ((n >> 1) as i64 ^ -((n & 1) as i64)).to_string()
            }
            (7, 0) => (cursor.number()? != 0).to_string(),
            _ => {
                cursor.skip(wire)?;
                value
            }
        };
    }
    Ok(value)
}
fn words(bytes: &[u8], out: &mut Vec<u32>, budget: &mut usize) -> Result<(), String> {
    let mut cursor = Cursor::new(bytes);
    while !cursor.done() {
        if *budget == 0 {
            return Err("Vector tile is too complex.".into());
        }
        *budget -= 1;
        out.push(u32::try_from(cursor.number()?).map_err(|_| "Invalid vector tile geometry.")?);
    }
    Ok(())
}
fn feature(
    bytes: &[u8],
    keys: &[String],
    values: &[String],
    budget: &mut usize,
    property_budget: &mut usize,
) -> Result<Feature, String> {
    let mut cursor = Cursor::new(bytes);
    let mut tags = Vec::new();
    let mut geometry = Vec::new();
    let mut kind = 0;
    while !cursor.done() {
        match cursor.field()? {
            (2, 2) => words(cursor.data()?, &mut tags, budget)?,
            (3, 0) => {
                kind = u8::try_from(cursor.number()?).map_err(|_| "Invalid geometry type.")?;
            }
            (4, 2) => words(cursor.data()?, &mut geometry, budget)?,
            (_, wire) => cursor.skip(wire)?,
        }
    }
    if tags.len() > 256 || tags.len() % 2 != 0 || kind > 3 {
        return Err("Invalid vector tile feature.".into());
    }
    let mut properties = BTreeMap::new();
    for pair in tags.chunks_exact(2) {
        let key = keys
            .get(pair[0] as usize)
            .ok_or("Invalid vector tile tag.")?;
        let value = values
            .get(pair[1] as usize)
            .ok_or("Invalid vector tile tag.")?;
        let limit = match key.as_str() {
            "class" | "brunnel" | "rank" => 64,
            "name_en" | "name:latin" | "name" => 256,
            _ => continue,
        };
        if value.len() > limit {
            continue;
        }
        let size = key.len() + value.len();
        *property_budget = property_budget
            .checked_sub(size)
            .ok_or("Vector tile properties are too large.")?;
        properties.insert(key.clone(), value.clone());
    }
    Ok(Feature {
        kind,
        properties,
        geometry,
    })
}
pub fn decode(bytes: &[u8]) -> Result<Vec<Layer>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Vector tile is too large.".into());
    }
    let mut cursor = Cursor::new(bytes);
    let mut output = Vec::new();
    let mut features = 0;
    let mut budget = MAX_WORDS;
    let mut property_budget = MAX_BYTES;
    while !cursor.done() {
        let (field, wire) = cursor.field()?;
        if (field, wire) != (3, 2) {
            cursor.skip(wire)?;
            continue;
        }
        if output.len() >= 32 {
            return Err("Too many vector tile layers.".into());
        }
        let mut layer = Cursor::new(cursor.data()?);
        let mut name = String::new();
        let mut extent = 4096;
        let mut version = 1;
        let mut raw_features = Vec::new();
        let mut keys = Vec::new();
        let mut values = Vec::new();
        while !layer.done() {
            match layer.field()? {
                (1, 2) => {
                    name = string(layer.data()?)?;
                }
                (2, 2) => {
                    features += 1;
                    if features > MAX_FEATURES {
                        return Err("Too many vector tile features.".into());
                    }
                    raw_features.push(layer.data()?);
                }
                (3, 2) => {
                    if keys.len() >= 1024 {
                        return Err("Too many vector tile keys.".into());
                    }
                    keys.push(string(layer.data()?)?);
                }
                (4, 2) => {
                    if values.len() >= 65536 {
                        return Err("Too many vector tile values.".into());
                    }
                    values.push(value(layer.data()?)?);
                }
                (5, 0) => {
                    extent = u32::try_from(layer.number()?).map_err(|_| "Invalid tile extent.")?;
                }
                (15, 0) => version = layer.number()?,
                (_, wire) => layer.skip(wire)?,
            }
        }
        if name.len() > 128 || !(256..=65536).contains(&extent) || !(1..=2).contains(&version) {
            return Err("Unsupported vector tile layer.".into());
        }
        let features = raw_features
            .into_iter()
            .map(|b| feature(b, &keys, &values, &mut budget, &mut property_budget))
            .collect::<Result<_, _>>()?;
        output.push(Layer {
            name,
            extent,
            features,
        });
    }
    if output.is_empty() {
        return Err("Empty vector tile response.".into());
    }
    Ok(output)
}

pub fn paths(geometry: &[u32]) -> Result<Vec<Vec<(i64, i64)>>, String> {
    let mut paths = Vec::<Vec<(i64, i64)>>::new();
    let (mut x, mut y, mut at) = (0i64, 0i64, 0usize);
    while at < geometry.len() {
        let command = geometry[at];
        at += 1;
        let (kind, count) = (command & 7, command >> 3);
        if count == 0 || count as usize > MAX_WORDS {
            return Err("Invalid vector geometry command.".into());
        }
        match kind {
            1 | 2 => {
                for _ in 0..count {
                    let dx = *geometry.get(at).ok_or("Truncated vector geometry.")?;
                    let dy = *geometry.get(at + 1).ok_or("Truncated vector geometry.")?;
                    at += 2;
                    x = x
                        .checked_add(i64::from(dx >> 1) ^ -i64::from(dx & 1))
                        .ok_or("Geometry overflow.")?;
                    y = y
                        .checked_add(i64::from(dy >> 1) ^ -i64::from(dy & 1))
                        .ok_or("Geometry overflow.")?;
                    if x.abs() > 1 << 24 || y.abs() > 1 << 24 {
                        return Err("Invalid geometry coordinate.".into());
                    }
                    if kind == 1 {
                        paths.push(Vec::new());
                    }
                    paths
                        .last_mut()
                        .ok_or("Invalid vector geometry path.")?
                        .push((x, y));
                }
            }
            7 => {
                if count != 1 {
                    return Err("Invalid closed geometry.".into());
                }
                let path = paths.last_mut().ok_or("Invalid closed geometry.")?;
                let first = *path.first().ok_or("Empty vector geometry path.")?;
                path.push(first);
            }
            _ => return Err("Invalid vector geometry command.".into()),
        }
    }
    Ok(paths)
}
