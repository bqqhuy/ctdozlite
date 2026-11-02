#[path = "../build_support/windows_resources.rs"]
mod windows_resources;

#[test]
fn arm64_manifest_requests_standard_user_without_x64_identity() {
    let manifest = windows_resources::build_windows_manifest("0.0.3");
    assert!(manifest.contains("processorArchitecture=\"arm64\""));
    assert!(!manifest.contains("amd64"));
    assert!(manifest.contains("level=\"asInvoker\" uiAccess=\"false\""));
    assert!(manifest.contains("version=\"0.0.3.0\""));
    assert!(manifest.contains("bqqhuy.ctdozlite"));
}

#[test]
fn explorer_version_resource_has_valid_fixed_header_and_cargo_version() {
    let resource = windows_resources::build_vs_version_info("0.0.3");
    assert_eq!(
        u16::from_le_bytes(resource[..2].try_into().unwrap()) as usize,
        resource.len()
    );
    let key: Vec<u8> = "VS_VERSION_INFO\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(&resource[6..6 + key.len()], key);
    let fixed = (6 + key.len() + 3) & !3;
    assert_eq!(
        u32::from_le_bytes(resource[fixed..fixed + 4].try_into().unwrap()),
        0xFEEF04BD
    );
    assert_eq!(
        u32::from_le_bytes(resource[fixed + 12..fixed + 16].try_into().unwrap()),
        3 << 16
    );
    for property in [
        "FileVersion",
        "ProductVersion",
        "0.0.3",
        "OriginalFilename",
        "ctdozlite.exe",
    ] {
        let encoded: Vec<u8> = property.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(
            resource.windows(encoded.len()).any(|w| w == encoded),
            "missing Explorer property {property}"
        );
    }
}
