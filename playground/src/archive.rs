use serde::Deserialize;

#[derive(Deserialize)]
pub struct ArchiveFile {
    pub path: String,
    pub content: String,
}

pub fn zip(files: &[ArchiveFile]) -> Result<Vec<u8>, String> {
    if files.len() > 32 {
        return Err("Too many files".into());
    }
    let mut output = Vec::new();
    let mut central = Vec::new();
    for file in files {
        let path = file.path.as_bytes();
        if path.is_empty()
            || path.len() > u16::MAX as usize
            || file.path.starts_with('/')
            || file
                .path
                .split('/')
                .any(|part| part == ".." || part.is_empty())
        {
            return Err(format!("Invalid archive path: {}", file.path));
        }
        let size = u32::try_from(file.content.len()).map_err(|_| "File is too large")?;
        let offset = u32::try_from(output.len()).map_err(|_| "Archive is too large")?;
        let crc = crc32(file.content.as_bytes());
        write_u32(&mut output, 0x0403_4b50);
        write_u16(&mut output, 20);
        write_u16(&mut output, 0x0800);
        write_u16(&mut output, 0);
        write_u16(&mut output, 0);
        write_u16(&mut output, 0);
        write_u32(&mut output, crc);
        write_u32(&mut output, size);
        write_u32(&mut output, size);
        write_u16(&mut output, path.len() as u16);
        write_u16(&mut output, 0);
        output.extend_from_slice(path);
        output.extend_from_slice(file.content.as_bytes());

        write_u32(&mut central, 0x0201_4b50);
        write_u16(&mut central, 20);
        write_u16(&mut central, 20);
        write_u16(&mut central, 0x0800);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, crc);
        write_u32(&mut central, size);
        write_u32(&mut central, size);
        write_u16(&mut central, path.len() as u16);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, 0);
        write_u32(&mut central, offset);
        central.extend_from_slice(path);
    }
    let directory_start = u32::try_from(output.len()).map_err(|_| "Archive is too large")?;
    let directory_size = u32::try_from(central.len()).map_err(|_| "Archive is too large")?;
    output.extend_from_slice(&central);
    write_u32(&mut output, 0x0605_4b50);
    write_u16(&mut output, 0);
    write_u16(&mut output, 0);
    write_u16(&mut output, files.len() as u16);
    write_u16(&mut output, files.len() as u16);
    write_u32(&mut output, directory_size);
    write_u32(&mut output, directory_start);
    write_u16(&mut output, 0);
    Ok(output)
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_has_local_and_central_records() {
        let bytes = zip(&[ArchiveFile {
            path: "src/main.rs".into(),
            content: "fn main() {}".into(),
        }])
        .unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        assert!(bytes.windows(4).any(|part| part == b"PK\x01\x02"));
        assert!(bytes.windows(4).any(|part| part == b"PK\x05\x06"));
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        assert!(zip(&[ArchiveFile {
            path: "../bad".into(),
            content: String::new()
        }])
        .is_err());
    }
}
