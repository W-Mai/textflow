use std::collections::BTreeMap;

fn bytes(data: &[u8], offset: usize, length: usize) -> Result<&[u8], String> {
    data.get(offset..offset.checked_add(length).ok_or("font offset overflow")?)
        .ok_or_else(|| "font table is truncated".into())
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(
        bytes(data, offset, 2)?.try_into().unwrap(),
    ))
}

fn i16_at(data: &[u8], offset: usize) -> Result<i16, String> {
    Ok(i16::from_be_bytes(
        bytes(data, offset, 2)?.try_into().unwrap(),
    ))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(
        bytes(data, offset, 4)?.try_into().unwrap(),
    ))
}

fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Result<&'a [u8], String> {
    let count = usize::from(u16_at(font, 4)?);
    for index in 0..count {
        let entry = 12 + index * 16;
        if bytes(font, entry, 4)? == tag {
            return bytes(
                font,
                u32_at(font, entry + 8)? as usize,
                u32_at(font, entry + 12)? as usize,
            );
        }
    }
    Err(format!("font table {:?} is missing", tag))
}

fn glyph(cmap: &[u8], code: u16) -> Result<Option<u16>, String> {
    let records = usize::from(u16_at(cmap, 2)?);
    let mut selected = None;
    for index in 0..records {
        let entry = 4 + index * 8;
        if (u16_at(cmap, entry)?, u16_at(cmap, entry + 2)?) == (3, 1) {
            selected = Some(u32_at(cmap, entry + 4)? as usize);
            break;
        }
    }
    let subtable = bytes(cmap, selected.ok_or("Windows Unicode cmap is missing")?, 16)?;
    if u16_at(subtable, 0)? != 4 {
        return Err("expected cmap format 4".into());
    }
    let length = usize::from(u16_at(subtable, 2)?);
    let subtable = bytes(cmap, selected.unwrap(), length)?;
    let count = usize::from(u16_at(subtable, 6)? / 2);
    let ends = 14;
    let starts = ends + count * 2 + 2;
    let deltas = starts + count * 2;
    let ranges = deltas + count * 2;
    for index in 0..count {
        let start = u16_at(subtable, starts + index * 2)?;
        let end = u16_at(subtable, ends + index * 2)?;
        if code < start || code > end {
            continue;
        }
        let delta = u16_at(subtable, deltas + index * 2)?;
        let range = usize::from(u16_at(subtable, ranges + index * 2)?);
        let value = if range == 0 {
            code.wrapping_add(delta)
        } else {
            let offset = ranges + index * 2 + range + usize::from(code - start) * 2;
            let mapped = u16_at(subtable, offset)?;
            if mapped == 0 {
                0
            } else {
                mapped.wrapping_add(delta)
            }
        };
        return Ok((value != 0).then_some(value));
    }
    Ok(None)
}

fn normalized(value: i32, units: i32) -> i32 {
    let scaled = value * 1000;
    if scaled < 0 {
        (scaled - units / 2) / units
    } else {
        (scaled + units / 2) / units
    }
}

pub fn generate(font: &[u8]) -> Result<String, String> {
    let head = table(font, b"head")?;
    let hhea = table(font, b"hhea")?;
    let hmtx = table(font, b"hmtx")?;
    let cmap = table(font, b"cmap")?;
    let kern = table(font, b"kern")?;
    let units = i32::from(u16_at(head, 18)?);
    if units == 0 {
        return Err("font units per em is zero".into());
    }
    let metrics_count = usize::from(u16_at(hhea, 34)?);
    if metrics_count == 0 {
        return Err("font has no horizontal metrics".into());
    }
    let mut advances = BTreeMap::new();
    let mut glyph_chars = BTreeMap::new();
    for code in (0x20..=0x7e).chain([0x00a0, 0x2026, 0xfb01, 0xfb02]) {
        if let Some(id) = glyph(cmap, code)? {
            let metric = usize::from(id).min(metrics_count - 1) * 4;
            let advance = normalized(i32::from(u16_at(hmtx, metric)?), units);
            advances.insert(code, advance);
            glyph_chars.insert(id, code);
        }
    }
    for required in [b'f' as u16, b'i' as u16, b'l' as u16, 0xfb01, 0xfb02] {
        if !advances.contains_key(&required) {
            return Err(format!("required Latin glyph U+{required:04X} is missing"));
        }
    }
    if u16_at(kern, 0)? != 0 {
        return Err("unsupported kern table version".into());
    }
    let count = usize::from(u16_at(kern, 2)?);
    let mut offset = 4;
    let mut pairs = BTreeMap::new();
    for _ in 0..count {
        let length = usize::from(u16_at(kern, offset + 2)?);
        let subtable = bytes(kern, offset, length)?;
        let coverage = u16_at(subtable, 4)?;
        if coverage & 0xff00 == 0 && coverage & 1 != 0 && coverage & 6 == 0 {
            let pair_count = usize::from(u16_at(subtable, 6)?);
            for index in 0..pair_count {
                let entry = 14 + index * 6;
                let left = u16_at(subtable, entry)?;
                let right = u16_at(subtable, entry + 2)?;
                if let (Some(&left), Some(&right)) =
                    (glyph_chars.get(&left), glyph_chars.get(&right))
                {
                    let value = normalized(i32::from(i16_at(subtable, entry + 4)?), units);
                    if value != 0 {
                        pairs.insert((left, right), value);
                    }
                }
            }
        }
        offset += length;
    }
    if pairs.get(&(b'T' as u16, b'o' as u16)).copied().unwrap_or(0) == 0 {
        return Err("expected To kerning pair is missing".into());
    }
    let mut source = format!(
        "pub const LATIN_ASCENDER: i32 = {};\npub const LATIN_DESCENDER: i32 = {};\npub const LATIN_LINE_GAP: i32 = {};\n",
        normalized(i32::from(i16_at(hhea, 4)?), units),
        normalized(i32::from(i16_at(hhea, 6)?), units),
        normalized(i32::from(i16_at(hhea, 8)?), units),
    );
    source.push_str("pub const LATIN_ADVANCES: &[(u16, i32)] = &[\n");
    for (code, advance) in advances {
        source.push_str(&format!("    ({code}, {advance}),\n"));
    }
    source.push_str("];\npub const LATIN_KERNING: &[((u16, u16), i32)] = &[\n");
    for ((left, right), value) in pairs {
        source.push_str(&format!("    (({left}, {right}), {value}),\n"));
    }
    source.push_str("];\n");
    Ok(source)
}
