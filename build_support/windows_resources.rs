//! Windows ARM64 version and application manifest resources.

fn align4(buf: &mut Vec<u8>) {
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
}
fn push_utf16_z(buf: &mut Vec<u8>, s: &str) {
    for u in s.encode_utf16() {
        buf.extend_from_slice(&u.to_le_bytes());
    }
    buf.extend_from_slice(&0u16.to_le_bytes());
}

fn build_version_node(key: &str, w_type: u16, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // placeholder wLength
    let val_len = if w_type == 1 {
        (value.len() / 2) as u16
    } else {
        value.len() as u16
    };
    out.extend_from_slice(&val_len.to_le_bytes());
    out.extend_from_slice(&w_type.to_le_bytes());
    push_utf16_z(&mut out, key);
    align4(&mut out);
    if !value.is_empty() {
        out.extend_from_slice(value);
        align4(&mut out);
    }
    for (idx, child) in children.iter().enumerate() {
        out.extend_from_slice(child);
        if idx + 1 < children.len() {
            align4(&mut out);
        }
    }
    let total_len = out.len() as u16;
    out[0..2].copy_from_slice(&total_len.to_le_bytes());
    out
}

/// Build version resources using the Cargo package version for Explorer properties.
#[cfg(test)]
pub fn build_vs_version_info(pkg_ver: &str) -> Vec<u8> {
    build_vs_version_info_with_product(pkg_ver, pkg_ver)
}

/// Build numeric file metadata with a separate human-readable release label.
pub fn build_vs_version_info_with_product(pkg_ver: &str, product_version: &str) -> Vec<u8> {
    let mut parts = pkg_ver
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u16>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    let build = 0u16;

    let file_ver_ms = ((major as u32) << 16) | (minor as u32);
    let file_ver_ls = ((patch as u32) << 16) | (build as u32);

    // VS_FIXEDFILEINFO (52 bytes)
    let mut fixed = Vec::with_capacity(52);
    fixed.extend_from_slice(&0xFEEF04BDu32.to_le_bytes()); // dwSignature
    fixed.extend_from_slice(&0x00010000u32.to_le_bytes()); // dwStrucVersion
    fixed.extend_from_slice(&file_ver_ms.to_le_bytes()); // dwFileVersionMS
    fixed.extend_from_slice(&file_ver_ls.to_le_bytes()); // dwFileVersionLS
    fixed.extend_from_slice(&file_ver_ms.to_le_bytes()); // dwProductVersionMS
    fixed.extend_from_slice(&file_ver_ls.to_le_bytes()); // dwProductVersionLS
    fixed.extend_from_slice(&0x0000003Fu32.to_le_bytes()); // dwFileFlagsMask
    fixed.extend_from_slice(&0x00000000u32.to_le_bytes()); // dwFileFlags
    fixed.extend_from_slice(&0x00040004u32.to_le_bytes()); // dwFileOS = VOS_NT_WINDOWS32
    fixed.extend_from_slice(&0x00000001u32.to_le_bytes()); // dwFileType = VFT_APP
    fixed.extend_from_slice(&0x00000000u32.to_le_bytes()); // dwFileSubtype
    fixed.extend_from_slice(&0x00000000u32.to_le_bytes()); // dwFileDateMS
    fixed.extend_from_slice(&0x00000000u32.to_le_bytes()); // dwFileDateLS

    let ver_quad = format!("{major}.{minor}.{patch}.{build}");
    let string_pairs = [
        ("CompanyName", "bqqhuy"),
        ("FileDescription", "ctdozlite Native Desktop Client"),
        ("FileVersion", ver_quad.as_str()),
        ("InternalName", "ctdozlite.exe"),
        (
            "LegalCopyright",
            "Copyright (c) 2026 ctdoteam1110 (MIT OR Apache-2.0)",
        ),
        ("OriginalFilename", "ctdozlite.exe"),
        ("ProductName", "ctdozlite"),
        ("ProductVersion", product_version),
    ];

    let mut string_nodes = Vec::with_capacity(string_pairs.len());
    for (k, v) in string_pairs {
        let mut val_utf16 = Vec::new();
        push_utf16_z(&mut val_utf16, v);
        string_nodes.push(build_version_node(k, 1, &val_utf16, &[]));
    }

    let string_table = build_version_node("040904B0", 1, &[], &string_nodes);
    let string_file_info = build_version_node("StringFileInfo", 1, &[], &[string_table]);

    let translation_val = [0x09, 0x04, 0xB0, 0x04]; // 0x0409 (en-US), 1200 (UTF-16)
    let var_node = build_version_node("Translation", 0, &translation_val, &[]);
    let var_file_info = build_version_node("VarFileInfo", 1, &[], &[var_node]);

    build_version_node(
        "VS_VERSION_INFO",
        0,
        &fixed,
        &[string_file_info, var_file_info],
    )
}

/// Declare the ARM64 target, standard-user privileges and Windows compatibility.
pub fn build_windows_manifest(pkg_ver: &str) -> String {
    let mut parts = pkg_ver
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .filter_map(|p| p.parse::<u16>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="bqqhuy.ctdozlite" version="{major}.{minor}.{patch}.0" processorArchitecture="arm64"/>
  <description>ctdozlite Native Desktop Client</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
      <supportedOS Id="{{1f676c76-80e1-4239-95bb-83d0f6d0da78}}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>"#
    )
}
