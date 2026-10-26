use std::{env, fs, path::PathBuf};

// Emit the standard Win32 .res layout directly so icon builds do not require rc.exe.
// https://learn.microsoft.com/en-us/windows/win32/menurc/resource-file-formats
pub fn link_icon(binary: &str) {
    println!("cargo:rerun-if-changed=../../build/icon_resource.rs");
    println!("cargo:rerun-if-changed=../achimeld-app/assets/achimeld.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // Read at build-script execution time: rerun-if-changed may rerun a cached
    // build-script executable without refreshing bytes embedded by include_bytes!.
    let icon_path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../achimeld-app/assets/achimeld.ico");
    let icon = fs::read(icon_path).expect("read application icon asset");
    assert_eq!(&icon[..4], &[0, 0, 1, 0], "expected an ICO asset");
    let count = u16::from_le_bytes(icon[4..6].try_into().unwrap());
    let mut resource = Vec::new();
    entry(&mut resource, 0, 0, &[]);
    let mut group = icon[..6].to_vec();
    for index in 0..count {
        let start = 6 + usize::from(index) * 16;
        let directory = &icon[start..start + 16];
        let size = u32::from_le_bytes(directory[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(directory[12..16].try_into().unwrap()) as usize;
        let id = index + 1;
        entry(&mut resource, 3, id, &icon[offset..offset + size]);
        group.extend_from_slice(&directory[..12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    entry(&mut resource, 14, 1, &group);
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("achimeld-icon.res");
    fs::write(&output, resource).expect("write application icon resource");
    println!("cargo:rustc-link-arg-bin={binary}={}", output.display());
}

fn entry(output: &mut Vec<u8>, kind: u16, id: u16, data: &[u8]) {
    output.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    output.extend_from_slice(&32_u32.to_le_bytes());
    for word in [0xffff, kind, 0xffff, id] {
        output.extend_from_slice(&word.to_le_bytes());
    }
    output.extend_from_slice(&0_u32.to_le_bytes()); // DataVersion
    output.extend_from_slice(&0_u16.to_le_bytes()); // MemoryFlags
    output.extend_from_slice(&0_u16.to_le_bytes()); // Neutral language
    output.extend_from_slice(&0_u32.to_le_bytes()); // Version
    output.extend_from_slice(&0_u32.to_le_bytes()); // Characteristics
    output.extend_from_slice(data);
    // DWORD alignment; keep this helper compatible with the bootstrapper's MSRV.
    while output.len() & 3 != 0 {
        output.push(0);
    }
}
