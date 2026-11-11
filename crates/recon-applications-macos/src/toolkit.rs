use std::os::unix::fs::FileExt;
use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
pub enum Toolkit {
    #[serde(rename = "appkit")]
    AppKit,
    #[serde(rename = "catalyst")]
    Catalyst,
}

impl Toolkit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppKit => "appkit",
            Self::Catalyst => "catalyst",
        }
    }
}

pub fn read(path: &Path) -> Result<Toolkit, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let bytes = read_load_commands(&file)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    platform(&bytes)
        .and_then(|platform| match platform {
            1 => Some(Toolkit::AppKit),
            6 => Some(Toolkit::Catalyst),
            _ => None,
        })
        .ok_or_else(|| {
            format!("{} has no readable macOS LC_BUILD_VERSION platform", path.display())
        })
}

fn read_load_commands(file: &std::fs::File) -> std::io::Result<Vec<u8>> {
    let magic = read_at(file, 0, 4)?;
    let offset = if magic == [0xca, 0xfe, 0xba, 0xbe] { fat_arm64_offset(file)? } else { 0 };
    let header = read_at(file, offset, 32)?;
    let command_bytes = le_u32(&header, 20).unwrap_or(0) as usize;
    if command_bytes > 16 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Mach-O load commands are too large",
        ));
    }
    let commands = read_at(file, offset + 32, command_bytes)?;
    let mut bytes = header;
    bytes.extend(commands);
    Ok(bytes)
}

fn fat_arm64_offset(file: &std::fs::File) -> std::io::Result<usize> {
    let count = be_u32(&read_at(file, 4, 4)?, 0).unwrap_or(0) as usize;
    if count > 64 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "fat binary has too many architectures",
        ));
    }
    for index in 0..count {
        let entry = read_at(file, 8 + index * 20, 20)?;
        if be_u32(&entry, 0) == Some(0x0100_000c) {
            return Ok(be_u32(&entry, 8).unwrap_or(0) as usize);
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "fat binary has no arm64 slice"))
}

fn read_at(file: &std::fs::File, offset: usize, length: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = vec![0; length];
    file.read_exact_at(&mut bytes, offset as u64)?;
    Ok(bytes)
}

fn platform(bytes: &[u8]) -> Option<u32> {
    let offset =
        if bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) { fat_arm64_slice(bytes)? } else { 0 };
    mach_o_platform(bytes.get(offset..)?)
}

fn fat_arm64_slice(bytes: &[u8]) -> Option<usize> {
    let architectures = be_u32(bytes, 4)? as usize;
    for index in 0..architectures {
        let entry = 8_usize.checked_add(index.checked_mul(20)?)?;
        if be_u32(bytes, entry)? == 0x0100_000c {
            return Some(be_u32(bytes, entry + 8)? as usize);
        }
    }
    None
}

fn mach_o_platform(bytes: &[u8]) -> Option<u32> {
    if !bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]) {
        return None;
    }
    let commands = le_u32(bytes, 16)? as usize;
    let mut offset = 32_usize;
    for _ in 0..commands {
        let command = le_u32(bytes, offset)?;
        let size = le_u32(bytes, offset + 4)? as usize;
        if size < 8 {
            return None;
        }
        if command == 0x32 {
            return le_u32(bytes, offset + 8);
        }
        offset = offset.checked_add(size)?;
    }
    None
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_reads_macos_and_catalyst_build_platforms() {
        for (platform, expected) in [(1_u32, Toolkit::AppKit), (6_u32, Toolkit::Catalyst)] {
            let mut bytes = vec![0; 48];
            bytes[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
            bytes[16..20].copy_from_slice(&1_u32.to_le_bytes());
            bytes[20..24].copy_from_slice(&16_u32.to_le_bytes());
            bytes[32..36].copy_from_slice(&0x32_u32.to_le_bytes());
            bytes[36..40].copy_from_slice(&16_u32.to_le_bytes());
            bytes[40..44].copy_from_slice(&platform.to_le_bytes());
            assert_eq!(mach_o_platform(&bytes), Some(platform));
            let path = std::env::temp_dir()
                .join(format!("recon-toolkit-{}-{platform}", std::process::id()));
            std::fs::write(&path, &bytes).unwrap();
            assert_eq!(read(&path).unwrap(), expected);
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn proposal0007_rejects_truncated_and_unknown_macho_data() {
        assert_eq!(platform(&[]), None);
        assert_eq!(platform(&[0xcf, 0xfa, 0xed, 0xfe]), None);
        assert_eq!(platform(&[0; 64]), None);
    }
}
