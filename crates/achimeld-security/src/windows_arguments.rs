//! Lossless Windows executable, argv and environment construction.
use crate::SecurityError;
use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

pub fn terminated(value: &OsStr) -> Result<Vec<u16>, SecurityError> {
    let mut units: Vec<u16> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(SecurityError("Embedded NUL in Windows launch input"));
    }
    units.push(0);
    Ok(units)
}

/// Construct argv without decoding paths or permitting one argument to add others.
/// argv[0] is a separately validated executable name, with different parser rules.
/// https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments
pub fn command_line(executable: &OsStr, arguments: &[&OsStr]) -> Result<Vec<u16>, SecurityError> {
    let executable = terminated(executable)?;
    let executable = &executable[..executable.len() - 1];
    if executable.is_empty() || executable.contains(&(b'"' as u16)) {
        return Err(SecurityError("Invalid Windows executable name"));
    }
    let mut result = vec![b'"' as u16];
    result.extend_from_slice(executable);
    result.push(b'"' as u16);
    for argument in arguments {
        let argument = terminated(argument)?;
        result.extend([b' ' as u16, b'"' as u16]);
        let mut slashes = 0;
        for &unit in &argument[..argument.len() - 1] {
            if unit == b'\\' as u16 {
                slashes += 1;
                continue;
            }
            let count = if unit == b'"' as u16 {
                2 * slashes + 1
            } else {
                slashes
            };
            result.extend(std::iter::repeat_n(b'\\' as u16, count));
            result.push(unit);
            slashes = 0;
        }
        result.extend(std::iter::repeat_n(b'\\' as u16, 2 * slashes));
        result.push(b'"' as u16);
        if result.len() >= 32767 {
            return Err(SecurityError("Windows command line exceeds its limit"));
        }
    }
    result.push(0);
    if result.len() > 32767 {
        return Err(SecurityError("Windows command line exceeds its limit"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};

    #[test]
    fn native_parser_recovers_exact_arguments_including_invalid_utf16() {
        let executable = OsString::from("C:\\path with spaces\\program.exe");
        let mut values: Vec<OsString> = [
            "",
            "space tab\t",
            "quote\"value",
            "trailing\\",
            "slashes\\\\\"quote",
            "é😀",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        values.push(OsString::from_wide(&[0xd800, b'"' as u16, b'\\' as u16]));
        let arguments: Vec<&OsStr> = values.iter().map(OsString::as_os_str).collect();
        let command = command_line(&executable, &arguments).unwrap();
        let mut count = 0;
        // SAFETY: Terminated command and count output live through parsing. The returned
        // vector and strings belong to one LocalAlloc block, freed after copying units.
        unsafe {
            let argv = CommandLineToArgvW(command.as_ptr(), &mut count);
            assert!(!argv.is_null());
            assert_eq!(count as usize, values.len() + 1);
            for (index, expected) in std::iter::once(&executable)
                .chain(values.iter())
                .enumerate()
            {
                let value = *argv.add(index);
                let mut size = 0;
                while *value.add(size) != 0 {
                    size += 1;
                }
                assert_eq!(
                    std::slice::from_raw_parts(value, size),
                    expected.encode_wide().collect::<Vec<_>>()
                );
            }
            LocalFree(argv.cast());
        }
    }

    #[test]
    fn malformed_or_oversized_launch_inputs_are_rejected() {
        assert!(command_line(OsStr::new("bad\"exe"), &[]).is_err());
        assert!(command_line(OsStr::new(""), &[]).is_err());
        assert!(command_line(OsStr::new("program.exe"), &[OsStr::new("bad\0arg")]).is_err());
        assert!(
            command_line(OsStr::new("program.exe"), &[OsStr::new(&"x".repeat(32767))]).is_err()
        );
    }
}
