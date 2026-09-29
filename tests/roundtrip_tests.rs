//! Byte-exact round-trip invariant: decode → parse → join → encode == original bytes

use wenv::utils::text::TextFormat;
use wenv::{get_parser, ShellType};

fn roundtrip(shell: ShellType, bytes: &[u8]) -> Vec<u8> {
    let (text, fmt) = TextFormat::decode(bytes).unwrap();
    let result = if text.is_empty() {
        Default::default()
    } else {
        get_parser(shell).parse(&text)
    };
    let mut out = String::new();
    for e in &result.entries {
        out.push_str(&e.value);
        out.push('\n');
    }
    fmt.encode(&out)
}

const COMMON: &[&[u8]] = &[
    b"",
    b"\n",
    b"alias a='b'",
    b"alias a='b'\n",
    b"alias a='b'\r\necho x\r\nexport A=1\r\n",
    b"\xEF\xBB\xBFalias a='b'\n",
    b"    alias ll='ls -la'\n\texport A=1\n  source ~/x\n",
    b"# comment\n\n\nalias a='b'\n\n",
    b"export FOO=\"\n",
    b"export FOO=\"\nline2\"\n",
    b"source '\n",
];

const BASH: &[&[u8]] = &[
    b"alias a='b'\nfoo() {\n  echo 1\n",
    b"if true; then\n  echo 1\n",
    b"while true; do\n  echo done\n  sleep 1\ndone\nalias z='y'\n",
    b"foo() {\n  echo }\n}\n}\nalias a='b'\n",
    b"alias x='\necho hi'\n",
    b"plugins=(\n  git\n",
];

const PWSH: &[&[u8]] = &[
    b"Set-Alias a b\nfunction f {\n  1\n",
    b"if ($a) {\r\n  1\r\n}\r\nSet-Alias a b\r\n",
    b"# c\r\nfunction f {\r\n  1\r\n}\r\n",
    b"$env:X = \"\n",
    b"    Set-Alias a b\n",
    b"\xFF\xFES\x00e\x00t\x00-\x00A\x00l\x00i\x00a\x00s\x00 \x00a\x00 \x00b\x00\n\x00",
];

#[test]
fn test_roundtrip_bash_unmodified_is_byte_exact() {
    for input in COMMON.iter().chain(BASH) {
        assert_eq!(
            roundtrip(ShellType::Bash, input),
            *input,
            "bash input {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn test_roundtrip_pwsh_unmodified_is_byte_exact() {
    for input in COMMON.iter().chain(PWSH) {
        assert_eq!(
            roundtrip(ShellType::PowerShell, input),
            *input,
            "pwsh input {:?}",
            String::from_utf8_lossy(input)
        );
    }
}
