/// Normal launches use the verified user-owned package. Protected sessions are
/// explicit and never fall back when installation or Windows policy is missing.
pub fn session_arguments(arguments: &[String]) -> Result<(bool, &[String]), String> {
    if arguments.first().map(String::as_str) != Some("--protected") {
        return Ok((false, arguments));
    }
    let remaining = &arguments[1..];
    if remaining.is_empty() || read_argument(remaining)?.is_some() {
        Ok((true, remaining))
    } else {
        Err("Usage: Achimeld.exe --protected [--read-steam APP_ID]".into())
    }
}

pub fn read_argument(arguments: &[String]) -> Result<Option<[String; 2]>, String> {
    if arguments.first().map(String::as_str) != Some("--read-steam") {
        return Ok(None);
    }
    if arguments.len() != 2 {
        return Err("Usage: Achimeld.exe --read-steam APP_ID".into());
    }
    let value = &arguments[1];
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.parse::<u32>().is_err()
    {
        return Err("Steam read requires one canonical nonzero App ID".into());
    }
    Ok(Some(["--read-steam".into(), value.clone()]))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_launch_is_explicit_and_cannot_wrap_policy_commands() {
        let empty = vec![];
        assert_eq!(session_arguments(&empty).unwrap(), (false, &[][..]));
        let protected = vec!["--protected".into()];
        assert_eq!(session_arguments(&protected).unwrap(), (true, &[][..]));
        let read = vec!["--protected".into(), "--read-steam".into(), "480".into()];
        assert_eq!(session_arguments(&read).unwrap(), (true, &read[1..]));
        for command in [
            "--protected",
            "--verify",
            "--install-policy",
            "--self-test",
            "other",
        ] {
            assert!(session_arguments(&["--protected".into(), command.into()]).is_err());
        }
        assert!(
            session_arguments(&["--protected".into(), "--read-steam".into(), "0".into()]).is_err()
        );
    }
    #[test]
    fn read_command_forwards_only_a_nonzero_numeric_app_id() {
        assert_eq!(
            read_argument(&["--read-steam".into(), "4873800".into()]).unwrap(),
            Some(["--read-steam".into(), "4873800".into()])
        );
        for argument in ["0", "0480", "-1", "480 --self-test", "4294967296", "abc"] {
            assert!(read_argument(&["--read-steam".into(), argument.into()]).is_err());
        }
        assert!(read_argument(&["--read-steam".into()]).is_err());
    }
}
