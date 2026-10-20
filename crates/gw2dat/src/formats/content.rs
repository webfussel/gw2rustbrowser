//! Game content manifests (PF `cntc`, chunk `Main`), exported as XML in the
//! same layout as the original browser.

use std::fmt::Write;

use super::pf::{Cursor, PackFile};
use crate::error::Result;
use crate::fourcc::fcc;

pub const MAIN: u32 = fcc(b"Main");

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Writes `<name ItemCount="n"><data>…</data>…</name>`, with `item` producing
/// the inner XML of each element.
fn list(
    out: &mut String,
    name: &str,
    c: &mut Cursor,
    mut item: impl FnMut(&mut Cursor, &mut String) -> Result<()>,
) -> Result<()> {
    let (count, mut at) = c.array()?;
    let _ = writeln!(out, "  <{name} ItemCount=\"{count}\">");
    for _ in 0..count {
        out.push_str("    <data>");
        item(&mut at, out)?;
        out.push_str("</data>\n");
    }
    let _ = writeln!(out, "  </{name}>");
    Ok(())
}

fn dwords(at: &mut Cursor, out: &mut String, names: &[&str]) -> Result<()> {
    for name in names {
        let _ = write!(out, "<{name} Type=\"dword\">{}</{name}>", at.u32()?);
    }
    Ok(())
}

pub fn to_xml(data: &[u8]) -> Result<String> {
    let file = PackFile::parse(data)?;
    let chunk = file.require_chunk(MAIN)?;
    let mut c = chunk.cursor();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Root>\n  <FileFormat>cntc</FileFormat>\n");
    let _ = writeln!(out, "  <flags Type=\"dword\">{}</flags>", c.u32()?);

    list(&mut out, "typeInfos", &mut c, |at, out| {
        dwords(at, out, &["guidOffset", "uidOffset", "dataIdOffset", "nameOffset"])?;
        let _ = write!(out, "<trackReferences Type=\"byte\">{}</trackReferences>", at.u8()?);
        Ok(())
    })?;
    list(&mut out, "namespaces", &mut c, |at, out| {
        let _ = write!(out, "<name Type=\"wstring\">{}</name>", escape(&at.wstring()?));
        dwords(at, out, &["domain", "parentIndex"])
    })?;
    list(&mut out, "fileRefs", &mut c, |at, out| {
        let _ = write!(out, "{}", at.file_ref()?.unwrap_or(0));
        Ok(())
    })?;
    list(&mut out, "indexEntries", &mut c, |at, out| {
        dwords(at, out, &["type", "offset", "namespaceIndex", "rootIndex"])
    })?;
    list(&mut out, "localOffsets", &mut c, |at, out| dwords(at, out, &["relocOffset"]))?;
    list(&mut out, "externalOffsets", &mut c, |at, out| dwords(at, out, &["relocOffset", "targetFileIndex"]))?;
    list(&mut out, "fileIndices", &mut c, |at, out| dwords(at, out, &["relocOffset"]))?;
    list(&mut out, "stringIndices", &mut c, |at, out| dwords(at, out, &["relocOffset"]))?;
    list(&mut out, "trackedReferences", &mut c, |at, out| {
        dwords(at, out, &["sourceOffset", "targetFileIndex", "targetOffset"])
    })?;
    list(&mut out, "strings", &mut c, |at, out| {
        out.push_str(&escape(&at.wstring()?));
        Ok(())
    })?;
    let content = c.byte_array()?;
    let _ = writeln!(out, "  <content ItemCount=\"{}\"/>", content.len());
    out.push_str("</Root>\n");
    Ok(out)
}
