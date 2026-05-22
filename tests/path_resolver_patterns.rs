use std::fs;
use tempfile::tempdir;
use wenv::config::path_resolver::{resolve_patterns, ResolvedPattern};

#[test]
fn single_existing_file_resolves_to_file_variant() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("a.sh");
    fs::write(&p, b"echo hi\n").unwrap();
    let patterns = vec![p.to_string_lossy().to_string()];
    let out = resolve_patterns(&patterns);
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::File { path, exists, .. } => {
            assert_eq!(path, &p);
            assert!(*exists);
        }
        _ => panic!("expected File, got {:?}", out[0]),
    }
}

#[test]
fn glob_pattern_resolves_to_dir_with_sorted_files() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("b.sh"), b"x").unwrap();
    fs::write(dir.path().join("a.sh"), b"x").unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob.clone()]);
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::Dir { original, files, .. } => {
            assert_eq!(original, &glob);
            assert_eq!(files.len(), 2);
            assert!(files[0].0.ends_with("a.sh"));
            assert!(files[1].0.ends_with("b.sh"));
        }
        _ => panic!("expected Dir"),
    }
}

#[test]
fn directory_pattern_without_glob_resolves_to_dir() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.sh"), b"echo\n").unwrap();
    let p = dir.path().to_string_lossy().to_string();
    let out = resolve_patterns(&[p]);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], ResolvedPattern::Dir { .. }));
}

#[test]
fn binary_files_filtered_from_dir_expansion() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("ok.sh"), b"echo\n").unwrap();
    fs::write(dir.path().join("bad.dat"), [0u8, 1, 2, 3]).unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob]);
    match &out[0] {
        ResolvedPattern::Dir { files, .. } => {
            assert_eq!(files.len(), 1);
            assert!(files[0].0.ends_with("ok.sh"));
        }
        _ => panic!("expected Dir"),
    }
}

#[test]
fn empty_glob_still_emits_dir_header() {
    let dir = tempdir().unwrap();
    let glob = format!("{}/*", dir.path().display());
    let out = resolve_patterns(&[glob]);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], ResolvedPattern::Dir { .. }));
}

#[test]
fn unresolved_var_skipped_with_warning() {
    let out = resolve_patterns(&["$DEFINITELY_NOT_SET_XYZ/foo.sh".to_string()]);
    assert_eq!(out.len(), 0);
}

#[test]
fn duplicate_file_path_dropped_with_warning() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("a.sh");
    fs::write(&p, b"echo\n").unwrap();
    let glob = format!("{}/*", dir.path().display());
    let literal = p.to_string_lossy().to_string();
    let out = resolve_patterns(&[glob, literal.clone()]);
    // The glob captured a.sh first; literal should be dropped.
    assert_eq!(out.len(), 1);
    match &out[0] {
        ResolvedPattern::Dir { files, .. } => assert_eq!(files.len(), 1),
        _ => panic!("expected Dir"),
    }
}

#[test]
fn display_has_var_suffix_when_var_bearing() {
    std::env::set_var("WENV_TEST_VAR_A", "/tmp");
    let out = resolve_patterns(&["$WENV_TEST_VAR_A/wenv_synth.sh".to_string()]);
    assert_eq!(out.len(), 1);
    let s = format!("{}", out[0]);
    assert!(s.contains("($WENV_TEST_VAR_A/wenv_synth.sh)"), "got: {}", s);
    std::env::remove_var("WENV_TEST_VAR_A");
}
