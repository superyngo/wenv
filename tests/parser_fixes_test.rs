use wenv::model::ShellType;
use wenv::parser::get_parser;

#[test]
fn test_export_blank_function_not_merged() {
    // Test case 1: export + blank + function should NOT merge
    let content = r#"export HISTSIZE=20000

function dasd (){
  echo123
}

"#;

    let parser = get_parser(ShellType::Bash);
    let result = parser.parse(content);

    // Should have 2 entries: env (with trailing blank) and function (with trailing blank)
    // This is correct - they are NOT merged together
    assert_eq!(
        result.entries.len(),
        2,
        "Should have 2 entries (not merged)"
    );

    // First entry: EnvVar (with trailing blank L2)
    assert_eq!(result.entries[0].entry_type, wenv::model::EntryType::EnvVar);
    assert_eq!(result.entries[0].name, "HISTSIZE");
    assert_eq!(result.entries[0].line_number, Some(1));
    assert_eq!(
        result.entries[0].end_line,
        Some(2),
        "Env should absorb trailing blank L2"
    );

    // Second entry: Function (with trailing blank L6)
    assert_eq!(
        result.entries[1].entry_type,
        wenv::model::EntryType::Function
    );
    assert_eq!(result.entries[1].name, "dasd");
    assert_eq!(result.entries[1].line_number, Some(3));
    assert_eq!(
        result.entries[1].end_line,
        Some(6),
        "Function should absorb trailing blank L6"
    );
}

#[test]
fn test_multi_comment_not_merged_with_code() {
    // Test case 2: Multiple comments should NOT merge with following code
    let content = r#"# 後綴別名設定
# alias -s {md,txt,json}=vim
# alias -s py=python

# 讓 Tab 補全更聰明
autoload -Uz compinit && compinit
"#;

    let parser = get_parser(ShellType::Bash);
    let result = parser.parse(content);

    // Should have 2 entries: comment (L1-L5) and code (L6-L7)
    // Comment L1-L3 absorbs blank L4, then continues to absorb comment L5
    // This is multi-comment so it does NOT merge with code L6
    assert_eq!(
        result.entries.len(),
        2,
        "Should have 2 entries (comment did NOT merge with code)"
    );

    // First entry: Comment (multi-line L1-L5)
    assert_eq!(
        result.entries[0].entry_type,
        wenv::model::EntryType::Comment
    );
    assert_eq!(result.entries[0].line_number, Some(1));
    assert_eq!(result.entries[0].end_line, Some(5), "Comment absorbs L1-L5");

    // Second entry: Code (with trailing newline)
    assert_eq!(result.entries[1].entry_type, wenv::model::EntryType::Code);
    assert_eq!(result.entries[1].line_number, Some(6));
    assert_eq!(
        result.entries[1].end_line,
        Some(6),
        "Code L6 with no trailing blank"
    );
}

#[test]
fn test_function_absorbs_trailing_blanks() {
    // Test case 3: function + trailing blanks
    let content = r#"greet() {
  echo hello
}


alias foo='bar'
"#;

    let parser = get_parser(ShellType::Bash);
    let result = parser.parse(content);

    // Should have 2 entries: function (L1-L5 with trailing blanks) and alias (L6 only)
    assert_eq!(result.entries.len(), 2, "Should have 2 entries");

    // First entry: Function (with trailing blanks L4-L5)
    assert_eq!(
        result.entries[0].entry_type,
        wenv::model::EntryType::Function
    );
    assert_eq!(result.entries[0].name, "greet");
    assert_eq!(result.entries[0].line_number, Some(1));
    assert_eq!(
        result.entries[0].end_line,
        Some(5),
        "Function should absorb trailing blanks L4-L5"
    );

    // Second entry: Alias (no trailing blank - file ends after L6)
    assert_eq!(result.entries[1].entry_type, wenv::model::EntryType::Alias);
    assert_eq!(result.entries[1].name, "foo");
    assert_eq!(result.entries[1].line_number, Some(6));
    assert_eq!(
        result.entries[1].end_line,
        Some(6),
        "Alias L6 only - final newline is not a separate line"
    );
}
