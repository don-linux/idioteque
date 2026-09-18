//! Strip de `libcef.so`: binutils / llvm-strip, o un recorte ELF64 little-endian puro.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const EI_CLASS: usize = 4;
const EI_DATA: usize = 5;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_SHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: usize = 56;
const SHF_ALLOC: u64 = 0x2;
const SHF_INFO_LINK: u64 = 0x40;
const SHT_RELA: u32 = 4;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHT_STRTAB: u32 = 3;
const COPY_BUF: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StripMethod {
    Binutils(String),
    Builtin,
}

#[derive(Debug, Clone)]
pub struct StripReport {
    pub method: StripMethod,
    pub before: u64,
    pub after: u64,
}

#[derive(Debug, Clone)]
pub struct ElfHeaderInfo {
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

struct SectionHeader {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

/// Recorta `libcef.so` in-place. Prueba `strip --strip-all`, luego `llvm-strip`,
/// y si ambos fallan el recorte ELF propio.
pub fn strip_libcef_in_place(path: &Path) -> Result<StripReport, String> {
    strip_libcef_with_tools(path, &["strip", "llvm-strip"])
}

fn strip_libcef_with_tools(path: &Path, tools: &[&str]) -> Result<StripReport, String> {
    let before = fs::metadata(path)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?
        .len();
    let tmp = strip_temp_path(path);
    let _ = fs::remove_file(&tmp);

    let mut method = None;
    for tool in tools {
        if run_strip_tool(tool, path, &tmp) {
            method = Some(StripMethod::Binutils((*tool).into()));
            break;
        }
    }
    let method = match method {
        Some(method) => method,
        None => {
            strip_elf_file(path, &tmp)?;
            StripMethod::Builtin
        }
    };

    parse_elf_header(&tmp).map_err(|error| {
        let _ = fs::remove_file(&tmp);
        format!("El libcef stripeado no es un ELF válido: {error}")
    })?;

    fs::rename(&tmp, path).map_err(|error| {
        let _ = fs::remove_file(&tmp);
        format!("No se pudo reemplazar `{}`: {error}", path.display())
    })?;

    let after = fs::metadata(path)
        .map_err(|error| format!("No se pudo leer `{}`: {error}", path.display()))?
        .len();
    Ok(StripReport {
        method,
        before,
        after,
    })
}

pub fn strip_elf_file(input: &Path, output: &Path) -> Result<(), String> {
    let header = parse_elf_header(input)?;
    if header.e_shentsize as usize != ELF64_SHDR_SIZE {
        return Err(format!("e_shentsize inesperado: {}", header.e_shentsize));
    }
    if header.e_phentsize as usize != ELF64_PHDR_SIZE && header.e_phnum != 0 {
        return Err(format!("e_phentsize inesperado: {}", header.e_phentsize));
    }
    if header.e_shnum == 0 {
        return Err("ELF sin tabla de secciones".to_string());
    }

    let mut file = File::open(input)
        .map_err(|error| format!("No se pudo abrir `{}`: {error}", input.display()))?;

    let phdrs = read_program_headers(&mut file, &header)?;
    let sections = read_section_headers(&mut file, &header)?;
    if header.e_shstrndx as usize >= sections.len() {
        return Err("e_shstrndx fuera de rango".to_string());
    }
    let shstrtab = read_section_bytes(&mut file, &sections[header.e_shstrndx as usize])?;

    let names: Vec<String> = sections
        .iter()
        .map(|section| c_string_at(&shstrtab, section.sh_name as usize))
        .collect();

    let mut keep = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let keep_it =
            index == 0 || (section.sh_flags & SHF_ALLOC) != 0 || names[index] == ".shstrtab";
        if keep_it {
            keep.push(index);
        }
    }
    if keep.is_empty() {
        return Err("No quedaron secciones ELF".to_string());
    }

    let mut old_to_new = vec![None; sections.len()];
    for (new_index, &old_index) in keep.iter().enumerate() {
        old_to_new[old_index] = Some(new_index as u32);
    }

    let mut keep_end = 0u64;
    for &old_index in &keep {
        let section = &sections[old_index];
        if section.sh_type == SHT_NOBITS {
            continue;
        }
        if (section.sh_flags & SHF_ALLOC) == 0 && names[old_index] == ".shstrtab" {
            continue;
        }
        keep_end = keep_end.max(section.sh_offset.saturating_add(section.sh_size));
    }
    for phdr in &phdrs {
        keep_end = keep_end.max(phdr.0.saturating_add(phdr.1));
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("No se pudo crear `{}`: {error}", parent.display()))?;
    }
    let mut out = File::create(output)
        .map_err(|error| format!("No se pudo crear `{}`: {error}", output.display()))?;
    copy_range(&mut file, &mut out, 0, keep_end)?;

    let mut new_shstrtab = Vec::new();
    let mut new_name_off = Vec::with_capacity(keep.len());
    for &old_index in &keep {
        new_name_off.push(new_shstrtab.len() as u32);
        new_shstrtab.extend_from_slice(names[old_index].as_bytes());
        new_shstrtab.push(0);
    }

    let shstrtab_off = keep_end;
    out.write_all(&new_shstrtab)
        .map_err(|error| format!("No se pudo escribir .shstrtab: {error}"))?;

    let shoff = shstrtab_off + new_shstrtab.len() as u64;
    let shstrndx = keep
        .iter()
        .position(|&old| names[old] == ".shstrtab")
        .ok_or_else(|| "Falta .shstrtab entre las secciones conservadas".to_string())?;

    let mut new_headers = Vec::with_capacity(keep.len());
    for (new_index, &old_index) in keep.iter().enumerate() {
        let mut section = clone_section(&sections[old_index]);
        section.sh_name = new_name_off[new_index];
        section.sh_link = remap_index(section.sh_link, &old_to_new);
        if info_is_section_index(section.sh_type, section.sh_flags) {
            section.sh_info = remap_index(section.sh_info, &old_to_new);
        }
        if names[old_index] == ".shstrtab" {
            section.sh_type = SHT_STRTAB;
            section.sh_flags = 0;
            section.sh_addr = 0;
            section.sh_offset = shstrtab_off;
            section.sh_size = new_shstrtab.len() as u64;
            section.sh_link = 0;
            section.sh_info = 0;
            section.sh_addralign = 1;
            section.sh_entsize = 0;
        }
        let _ = new_index;
        new_headers.push(section);
    }

    for section in &new_headers {
        write_section_header(&mut out, section)?;
    }

    patch_elf_header(&mut out, shoff, keep.len() as u16, shstrndx as u16)?;
    out.flush()
        .map_err(|error| format!("No se pudo cerrar el ELF recortado: {error}"))?;
    Ok(())
}

pub fn parse_elf_header(path: &Path) -> Result<ElfHeaderInfo, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("No se pudo abrir `{}`: {error}", path.display()))?;
    let mut ehdr = [0u8; ELF64_EHDR_SIZE];
    file.read_exact(&mut ehdr)
        .map_err(|error| format!("No se pudo leer la cabecera ELF: {error}"))?;
    parse_ehdr(&ehdr)
}

fn parse_ehdr(ehdr: &[u8; ELF64_EHDR_SIZE]) -> Result<ElfHeaderInfo, String> {
    if ehdr[0..4] != ELF_MAGIC {
        return Err("no es un ELF (magia incorrecta)".to_string());
    }
    if ehdr[EI_CLASS] != ELFCLASS64 {
        return Err("solo se recortan ELF64".to_string());
    }
    if ehdr[EI_DATA] != ELFDATA2LSB {
        return Err("solo se recortan ELF little-endian".to_string());
    }
    Ok(ElfHeaderInfo {
        e_phoff: u64::from_le_bytes(ehdr[32..40].try_into().unwrap()),
        e_shoff: u64::from_le_bytes(ehdr[40..48].try_into().unwrap()),
        e_phentsize: u16::from_le_bytes(ehdr[54..56].try_into().unwrap()),
        e_phnum: u16::from_le_bytes(ehdr[56..58].try_into().unwrap()),
        e_shentsize: u16::from_le_bytes(ehdr[58..60].try_into().unwrap()),
        e_shnum: u16::from_le_bytes(ehdr[60..62].try_into().unwrap()),
        e_shstrndx: u16::from_le_bytes(ehdr[62..64].try_into().unwrap()),
    })
}

fn strip_temp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "libcef.so".into());
    name.push(".idioteque-strip.tmp");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

fn run_strip_tool(tool: &str, input: &Path, output: &Path) -> bool {
    let status = Command::new(tool)
        .args(["--strip-all", "-o"])
        .arg(output)
        .arg(input)
        .status();
    match status {
        Ok(status) if status.success() && output.is_file() => {
            // Una herramienta que “gana” pero escribe basura no debe tapar el fallback.
            if parse_elf_header(output).is_ok() {
                true
            } else {
                let _ = fs::remove_file(output);
                false
            }
        }
        _ => {
            let _ = fs::remove_file(output);
            false
        }
    }
}

fn read_program_headers(
    file: &mut File,
    header: &ElfHeaderInfo,
) -> Result<Vec<(u64, u64)>, String> {
    let mut out = Vec::with_capacity(header.e_phnum as usize);
    if header.e_phnum == 0 {
        return Ok(out);
    }
    file.seek(SeekFrom::Start(header.e_phoff))
        .map_err(|error| format!("No se pudieron leer los program headers: {error}"))?;
    let mut buf = vec![0u8; header.e_phentsize as usize];
    for _ in 0..header.e_phnum {
        file.read_exact(&mut buf)
            .map_err(|error| format!("Program header truncado: {error}"))?;
        let p_offset = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let p_filesz = u64::from_le_bytes(buf[32..40].try_into().unwrap());
        out.push((p_offset, p_filesz));
    }
    Ok(out)
}

fn read_section_headers(
    file: &mut File,
    header: &ElfHeaderInfo,
) -> Result<Vec<SectionHeader>, String> {
    file.seek(SeekFrom::Start(header.e_shoff))
        .map_err(|error| format!("No se pudieron leer las secciones ELF: {error}"))?;
    let mut out = Vec::with_capacity(header.e_shnum as usize);
    let mut buf = [0u8; ELF64_SHDR_SIZE];
    for _ in 0..header.e_shnum {
        file.read_exact(&mut buf)
            .map_err(|error| format!("Section header truncado: {error}"))?;
        out.push(parse_section_header(&buf));
    }
    Ok(out)
}

fn parse_section_header(buf: &[u8; ELF64_SHDR_SIZE]) -> SectionHeader {
    SectionHeader {
        sh_name: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
        sh_type: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
        sh_flags: u64::from_le_bytes(buf[8..16].try_into().unwrap()),
        sh_addr: u64::from_le_bytes(buf[16..24].try_into().unwrap()),
        sh_offset: u64::from_le_bytes(buf[24..32].try_into().unwrap()),
        sh_size: u64::from_le_bytes(buf[32..40].try_into().unwrap()),
        sh_link: u32::from_le_bytes(buf[40..44].try_into().unwrap()),
        sh_info: u32::from_le_bytes(buf[44..48].try_into().unwrap()),
        sh_addralign: u64::from_le_bytes(buf[48..56].try_into().unwrap()),
        sh_entsize: u64::from_le_bytes(buf[56..64].try_into().unwrap()),
    }
}

fn clone_section(section: &SectionHeader) -> SectionHeader {
    SectionHeader {
        sh_name: section.sh_name,
        sh_type: section.sh_type,
        sh_flags: section.sh_flags,
        sh_addr: section.sh_addr,
        sh_offset: section.sh_offset,
        sh_size: section.sh_size,
        sh_link: section.sh_link,
        sh_info: section.sh_info,
        sh_addralign: section.sh_addralign,
        sh_entsize: section.sh_entsize,
    }
}

fn write_section_header(out: &mut File, section: &SectionHeader) -> Result<(), String> {
    let mut buf = [0u8; ELF64_SHDR_SIZE];
    buf[0..4].copy_from_slice(&section.sh_name.to_le_bytes());
    buf[4..8].copy_from_slice(&section.sh_type.to_le_bytes());
    buf[8..16].copy_from_slice(&section.sh_flags.to_le_bytes());
    buf[16..24].copy_from_slice(&section.sh_addr.to_le_bytes());
    buf[24..32].copy_from_slice(&section.sh_offset.to_le_bytes());
    buf[32..40].copy_from_slice(&section.sh_size.to_le_bytes());
    buf[40..44].copy_from_slice(&section.sh_link.to_le_bytes());
    buf[44..48].copy_from_slice(&section.sh_info.to_le_bytes());
    buf[48..56].copy_from_slice(&section.sh_addralign.to_le_bytes());
    buf[56..64].copy_from_slice(&section.sh_entsize.to_le_bytes());
    out.write_all(&buf)
        .map_err(|error| format!("No se pudo escribir un section header: {error}"))
}

fn read_section_bytes(file: &mut File, section: &SectionHeader) -> Result<Vec<u8>, String> {
    if section.sh_type == SHT_NOBITS || section.sh_size == 0 {
        return Ok(Vec::new());
    }
    file.seek(SeekFrom::Start(section.sh_offset))
        .map_err(|error| format!("No se pudo leer .shstrtab: {error}"))?;
    let mut buf = vec![0u8; section.sh_size as usize];
    file.read_exact(&mut buf)
        .map_err(|error| format!(".shstrtab truncado: {error}"))?;
    Ok(buf)
}

fn c_string_at(data: &[u8], offset: usize) -> String {
    if offset >= data.len() {
        return String::new();
    }
    let end = data[offset..]
        .iter()
        .position(|b| *b == 0)
        .map(|rel| offset + rel)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[offset..end]).into_owned()
}

fn copy_range(input: &mut File, output: &mut File, start: u64, end: u64) -> Result<(), String> {
    if end < start {
        return Err("rango ELF inválido".to_string());
    }
    input
        .seek(SeekFrom::Start(start))
        .map_err(|error| format!("No se pudo leer el ELF: {error}"))?;
    let mut remaining = end - start;
    let mut buf = vec![0u8; COPY_BUF];
    while remaining > 0 {
        let chunk = remaining.min(buf.len() as u64) as usize;
        let n = input
            .read(&mut buf[..chunk])
            .map_err(|error| format!("No se pudo copiar el ELF: {error}"))?;
        if n == 0 {
            return Err("ELF truncado al copiar secciones ALLOC".to_string());
        }
        output
            .write_all(&buf[..n])
            .map_err(|error| format!("No se pudo escribir el ELF recortado: {error}"))?;
        remaining -= n as u64;
    }
    Ok(())
}

fn remap_index(old: u32, old_to_new: &[Option<u32>]) -> u32 {
    if old == 0 {
        return 0;
    }
    old_to_new.get(old as usize).copied().flatten().unwrap_or(0)
}

fn info_is_section_index(sh_type: u32, sh_flags: u64) -> bool {
    (sh_flags & SHF_INFO_LINK) != 0 || sh_type == SHT_REL || sh_type == SHT_RELA
}

fn patch_elf_header(out: &mut File, shoff: u64, shnum: u16, shstrndx: u16) -> Result<(), String> {
    out.seek(SeekFrom::Start(40))
        .map_err(|error| format!("No se pudo parchear la cabecera ELF: {error}"))?;
    out.write_all(&shoff.to_le_bytes())
        .map_err(|error| format!("No se pudo parchear e_shoff: {error}"))?;
    out.seek(SeekFrom::Start(60))
        .map_err(|error| format!("No se pudo parchear e_shnum: {error}"))?;
    out.write_all(&shnum.to_le_bytes())
        .map_err(|error| format!("No se pudo parchear e_shnum: {error}"))?;
    out.write_all(&shstrndx.to_le_bytes())
        .map_err(|error| format!("No se pudo parchear e_shstrndx: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    const SHF_EXECINSTR: u64 = 0x4;
    const SHT_NULL: u32 = 0;
    const SHT_PROGBITS: u32 = 1;
    const SHT_SYMTAB: u32 = 2;
    const SHT_DYNSYM: u32 = 11;
    const PT_LOAD: u32 = 1;

    struct Sec {
        name: &'static str,
        sh_type: u32,
        flags: u64,
        addr: u64,
        offset: u64,
        size: u64,
        link: u32,
        info: u32,
        data: Vec<u8>,
    }

    fn build_synthetic_elf() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        // Layout:
        // 0x0000 ehdr (64)
        // 0x0040 phdr (56)
        // 0x1000 .text  (16 x 0x90) ALLOC
        // 0x1010 .dynstr (8) ALLOC
        // 0x1018 .dynsym (24) ALLOC, link → .dynstr
        // 0x2000 .debug_info (64 x 0xff)
        // 0x2040 .symtab (24), link → .strtab
        // 0x2058 .strtab (8)
        // 0x2060 .shstrtab
        // aligned shdrs
        let text = vec![0x90u8; 16];
        let dynstr = b"abc\0def\0".to_vec();
        let dynsym = vec![0x11u8; 24];
        let debug = vec![0xffu8; 64];
        let symtab = vec![0x22u8; 24];
        let strtab = b"sym\0end\0".to_vec();

        let mut names: Vec<&str> = vec![
            "",
            ".text",
            ".dynstr",
            ".dynsym",
            ".debug_info",
            ".symtab",
            ".strtab",
            ".shstrtab",
        ];
        let mut shstrtab = Vec::new();
        let mut name_off = Vec::new();
        for name in &names {
            name_off.push(shstrtab.len() as u32);
            shstrtab.extend_from_slice(name.as_bytes());
            shstrtab.push(0);
        }
        let _ = &mut names;

        let sections = vec![
            Sec {
                name: "",
                sh_type: SHT_NULL,
                flags: 0,
                addr: 0,
                offset: 0,
                size: 0,
                link: 0,
                info: 0,
                data: vec![],
            },
            Sec {
                name: ".text",
                sh_type: SHT_PROGBITS,
                flags: SHF_ALLOC | SHF_EXECINSTR,
                addr: 0x1000,
                offset: 0x1000,
                size: text.len() as u64,
                link: 5, // .symtab (dropped) → 0 after strip
                info: 0,
                data: text.clone(),
            },
            Sec {
                name: ".dynstr",
                sh_type: SHT_STRTAB,
                flags: SHF_ALLOC,
                addr: 0x1010,
                offset: 0x1010,
                size: dynstr.len() as u64,
                link: 0,
                info: 0,
                data: dynstr.clone(),
            },
            Sec {
                name: ".dynsym",
                sh_type: SHT_DYNSYM,
                flags: SHF_ALLOC,
                addr: 0x1018,
                offset: 0x1018,
                size: dynsym.len() as u64,
                link: 2, // .dynstr (kept)
                info: 1,
                data: dynsym.clone(),
            },
            Sec {
                name: ".debug_info",
                sh_type: SHT_PROGBITS,
                flags: 0,
                addr: 0,
                offset: 0x2000,
                size: debug.len() as u64,
                link: 0,
                info: 0,
                data: debug.clone(),
            },
            Sec {
                name: ".symtab",
                sh_type: SHT_SYMTAB,
                flags: 0,
                addr: 0,
                offset: 0x2040,
                size: symtab.len() as u64,
                link: 6,
                info: 1,
                data: symtab.clone(),
            },
            Sec {
                name: ".strtab",
                sh_type: SHT_STRTAB,
                flags: 0,
                addr: 0,
                offset: 0x2058,
                size: strtab.len() as u64,
                link: 0,
                info: 0,
                data: strtab.clone(),
            },
            Sec {
                name: ".shstrtab",
                sh_type: SHT_STRTAB,
                flags: 0,
                addr: 0,
                offset: 0x2060,
                size: shstrtab.len() as u64,
                link: 0,
                info: 0,
                data: shstrtab.clone(),
            },
        ];

        let shoff = 0x2100u64;
        let file_size = shoff as usize + sections.len() * ELF64_SHDR_SIZE;
        let mut file = vec![0u8; file_size];

        // ELF header
        file[0..4].copy_from_slice(&ELF_MAGIC);
        file[4] = ELFCLASS64;
        file[5] = ELFDATA2LSB;
        file[6] = 1;
        file[16..18].copy_from_slice(&3u16.to_le_bytes()); // ET_DYN
        file[18..20].copy_from_slice(&62u16.to_le_bytes()); // EM_X86_64
        file[20..24].copy_from_slice(&1u32.to_le_bytes());
        file[24..32].copy_from_slice(&0x1000u64.to_le_bytes()); // e_entry
        file[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
        file[40..48].copy_from_slice(&shoff.to_le_bytes());
        file[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
        file[54..56].copy_from_slice(&56u16.to_le_bytes());
        file[56..58].copy_from_slice(&1u16.to_le_bytes());
        file[58..60].copy_from_slice(&64u16.to_le_bytes());
        file[60..62].copy_from_slice(&(sections.len() as u16).to_le_bytes());
        file[62..64].copy_from_slice(&7u16.to_le_bytes()); // .shstrtab idx

        // PT_LOAD covering alloc file bytes
        let ph_filesz = 0x1018u64 + 24;
        file[64..68].copy_from_slice(&PT_LOAD.to_le_bytes());
        file[68..72].copy_from_slice(&5u32.to_le_bytes()); // R+X
        file[72..80].copy_from_slice(&0u64.to_le_bytes()); // p_offset
        file[80..88].copy_from_slice(&0u64.to_le_bytes());
        file[88..96].copy_from_slice(&0u64.to_le_bytes());
        file[96..104].copy_from_slice(&ph_filesz.to_le_bytes());
        file[104..112].copy_from_slice(&ph_filesz.to_le_bytes());
        file[112..120].copy_from_slice(&0x1000u64.to_le_bytes());

        for (i, section) in sections.iter().enumerate() {
            if !section.data.is_empty() {
                let start = section.offset as usize;
                file[start..start + section.data.len()].copy_from_slice(&section.data);
            }
            let mut shdr = [0u8; ELF64_SHDR_SIZE];
            shdr[0..4].copy_from_slice(&name_off[i].to_le_bytes());
            shdr[4..8].copy_from_slice(&section.sh_type.to_le_bytes());
            shdr[8..16].copy_from_slice(&section.flags.to_le_bytes());
            shdr[16..24].copy_from_slice(&section.addr.to_le_bytes());
            shdr[24..32].copy_from_slice(&section.offset.to_le_bytes());
            shdr[32..40].copy_from_slice(&section.size.to_le_bytes());
            shdr[40..44].copy_from_slice(&section.link.to_le_bytes());
            shdr[44..48].copy_from_slice(&section.info.to_le_bytes());
            shdr[48..56].copy_from_slice(&1u64.to_le_bytes());
            shdr[56..64].copy_from_slice(&0u64.to_le_bytes());
            let at = shoff as usize + i * ELF64_SHDR_SIZE;
            file[at..at + ELF64_SHDR_SIZE].copy_from_slice(&shdr);
        }

        (file, text, dynstr)
    }

    fn load_sections(path: &Path) -> (ElfHeaderInfo, Vec<(String, SectionHeader)>) {
        let header = parse_elf_header(path).unwrap();
        let mut file = File::open(path).unwrap();
        let sections = read_section_headers(&mut file, &header).unwrap();
        let shstr = read_section_bytes(&mut file, &sections[header.e_shstrndx as usize]).unwrap();
        let named = sections
            .into_iter()
            .map(|section| {
                let name = c_string_at(&shstr, section.sh_name as usize);
                (name, section)
            })
            .collect();
        (header, named)
    }

    #[test]
    fn strip_elf_file_drops_debug_keeps_alloc_bytes_and_remaps_links() {
        let tmp = TempDir::new().unwrap();
        let input = tmp.path().join("in.so");
        let output = tmp.path().join("out.so");
        let (bytes, text, dynstr) = build_synthetic_elf();
        fs::write(&input, &bytes).unwrap();
        let before = bytes.len() as u64;

        strip_elf_file(&input, &output).expect("strip");
        let after = fs::metadata(&output).unwrap().len();
        assert!(after < before, "after={after} before={before}");

        let out_bytes = fs::read(&output).unwrap();
        assert_eq!(&out_bytes[0x1000..0x1010], text.as_slice());
        assert_eq!(&out_bytes[0x1010..0x1018], dynstr.as_slice());
        assert!(
            after < 0x2000,
            "debug payload should not be copied, after={after}"
        );

        let (header, sections) = load_sections(&output);
        assert_eq!(header.e_shnum as usize, sections.len());
        let names: Vec<&str> = sections.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["", ".text", ".dynstr", ".dynsym", ".shstrtab"]);
        assert_eq!(header.e_shstrndx, 4);

        let text_sec = &sections[1].1;
        assert_eq!(text_sec.sh_link, 0, "link to dropped .symtab becomes 0");
        let dynsym = &sections[3].1;
        assert_eq!(dynsym.sh_link, 2, "link to .dynstr remapped to 2");
        assert_eq!(sections[2].0, ".dynstr");
    }

    #[test]
    fn parse_elf_header_rejects_non_elf() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("not.elf");
        fs::write(&path, b"not-an-elf").unwrap();
        assert!(parse_elf_header(&path).is_err());
        assert!(strip_elf_file(&path, &tmp.path().join("out")).is_err());
    }

    #[cfg(unix)]
    fn write_exec(path: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn strip_libcef_falls_back_when_strip_tools_fail() {
        let tmp = TempDir::new().unwrap();
        let failing = tmp.path().join("strip-fail");
        write_exec(&failing, "#!/bin/sh\necho 'strip-fail ran' >&2\nexit 1\n");
        let input = tmp.path().join("libcef.so");
        let (bytes, text, dynstr) = build_synthetic_elf();
        fs::write(&input, &bytes).unwrap();
        let original = fs::read(&input).unwrap();

        let report = strip_libcef_with_tools(
            &input,
            &[failing.to_str().unwrap(), failing.to_str().unwrap()],
        )
        .expect("builtin fallback");
        assert_eq!(report.method, StripMethod::Builtin);
        parse_elf_header(&input).expect("sigue siendo ELF");
        let out = fs::read(&input).unwrap();
        assert_eq!(&out[0x1000..0x1010], text.as_slice());
        assert_eq!(&out[0x1010..0x1018], dynstr.as_slice());
        assert_ne!(out, original, "el fallback builtin debe reescribir el ELF");
        assert!(report.after < report.before);
    }

    #[cfg(unix)]
    #[test]
    fn strip_libcef_falls_back_when_strip_tool_writes_garbage() {
        let tmp = TempDir::new().unwrap();
        let garbage = tmp.path().join("strip-garbage");
        write_exec(
            &garbage,
            concat!(
                "#!/bin/sh\n",
                "out=''\n",
                "while [ $# -gt 0 ]; do\n",
                "  if [ \"$1\" = \"-o\" ]; then shift; out=\"$1\"; fi\n",
                "  shift\n",
                "done\n",
                "printf 'not-an-elf' > \"$out\"\n",
                "exit 0\n",
            ),
        );
        let input = tmp.path().join("libcef.so");
        let (bytes, text, _) = build_synthetic_elf();
        fs::write(&input, &bytes).unwrap();

        let report = strip_libcef_with_tools(&input, &[garbage.to_str().unwrap()])
            .expect("garbage tool must not block builtin");
        assert_eq!(report.method, StripMethod::Builtin);
        parse_elf_header(&input).expect("original replaced with valid ELF");
        let out = fs::read(&input).unwrap();
        assert_eq!(&out[0x1000..0x1010], text.as_slice());
        assert_ne!(&out[..], b"not-an-elf");
    }

    #[test]
    fn strip_elf_file_is_idempotent_on_already_stripped() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src.so");
        let once = tmp.path().join("once.so");
        let twice = tmp.path().join("twice.so");
        let (bytes, text, dynstr) = build_synthetic_elf();
        fs::write(&src, &bytes).unwrap();
        strip_elf_file(&src, &once).expect("first");
        parse_elf_header(&once).expect("stripped once");
        let once_bytes = fs::read(&once).unwrap();
        strip_elf_file(&once, &twice).expect("already-stripped");
        parse_elf_header(&twice).expect("stripped twice");
        let twice_bytes = fs::read(&twice).unwrap();
        assert_eq!(&twice_bytes[0x1000..0x1010], text.as_slice());
        assert_eq!(&twice_bytes[0x1010..0x1018], dynstr.as_slice());
        let (header, sections) = load_sections(&twice);
        let names: Vec<&str> = sections.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["", ".text", ".dynstr", ".dynsym", ".shstrtab"]);
        assert_eq!(header.e_shstrndx, 4);
        assert!(
            twice_bytes.len() <= once_bytes.len() + 256,
            "re-strip no debe hinchar el ELF: once={} twice={}",
            once_bytes.len(),
            twice_bytes.len()
        );
    }

    #[test]
    fn strip_libcef_in_place_on_already_stripped_stays_elf() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("libcef.so");
        let (bytes, text, dynstr) = build_synthetic_elf();
        fs::write(&path, &bytes).unwrap();
        strip_elf_file(&path, &tmp.path().join("pre.so")).unwrap();
        fs::copy(tmp.path().join("pre.so"), &path).unwrap();
        let before = fs::metadata(&path).unwrap().len();
        let report = strip_libcef_in_place(&path).expect("strip already-stripped");
        parse_elf_header(&path).expect("sigue ELF");
        let out = fs::read(&path).unwrap();
        assert_eq!(&out[0x1000..0x1010], text.as_slice());
        assert_eq!(&out[0x1010..0x1018], dynstr.as_slice());
        assert!(report.after > 64, "no debe quedar un header huérfano");
        assert!(
            report.after <= before + 256,
            "already-stripped no debe crecer: before={before} after={}",
            report.after
        );
    }

    #[test]
    fn strip_real_libcef_gated() {
        if std::env::var("IDIOTEQUE_CEF_REAL_LIBCEF").ok().as_deref() != Some("1") {
            return;
        }
        let src = Path::new("/workspace/src-tauri/.cef-sdk/152.0.6/cef_linux_x86_64/libcef.so");
        if !src.is_file() {
            return;
        }
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("libcef.so");
        strip_elf_file(src, &dest).expect("builtin strip of real libcef.so");
        let after = dest.metadata().unwrap().len();
        let expected = 267_902_104u64;
        let delta = after.abs_diff(expected);
        assert!(
            delta * 100 / expected <= 1,
            "stripped size {after} not within 1% of {expected} (delta {delta})"
        );
        parse_elf_header(&dest).expect("still ELF");

        if let Ok(output) = Command::new("readelf").arg("-S").arg(&dest).output() {
            assert!(
                output.status.success(),
                "readelf -S failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let sdk = src.parent().unwrap();
        let dest_str = dest.to_string_lossy().replace('\'', r#"'\''"#);
        let script = format!(
            "import ctypes, os; os.environ['LD_LIBRARY_PATH']='{}'; ctypes.CDLL('{}')",
            sdk.display(),
            dest_str
        );
        let py = Command::new("python3")
            .arg("-c")
            .arg(&script)
            .env("LD_LIBRARY_PATH", sdk)
            .output()
            .expect("python3 ctypes");
        assert!(
            py.status.success(),
            "ctypes.CDLL failed: stdout={} stderr={}",
            String::from_utf8_lossy(&py.stdout),
            String::from_utf8_lossy(&py.stderr)
        );
        eprintln!(
            "[cef-elf-strip] real libcef.so builtin strip: before={} after={after} expected={expected} delta={delta}",
            src.metadata().unwrap().len()
        );
    }
}
