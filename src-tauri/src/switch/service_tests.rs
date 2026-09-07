use super::*;
fn prepare(path: PathBuf) -> Prepared {
    atomic::write(&path, b"old").unwrap();
    let temp = atomic::stage(&path, b"new").unwrap();
    Prepared { path, old: Some(b"old".to_vec()), new: b"new".to_vec(), temp: Some(temp) }
}
#[test]
fn second_commit_failure_rolls_back_first() {
    let d = tempfile::tempdir().unwrap();
    let mut p = vec![prepare(d.path().join("a")), prepare(d.path().join("b"))];
    let result = commit_prepared(&mut p, |i, path, temp| {
        if i == 1 { Err(AppError::new("FILE_PERMISSION", "test")) } else { atomic::commit(path, temp) }
    });
    assert_eq!(result.unwrap_err().code, "FILE_PERMISSION");
    for f in p { assert_eq!(std::fs::read(f.path).unwrap(), b"old"); }
}
#[test]
fn rollback_never_overwrites_external_update() {
    let d = tempfile::tempdir().unwrap();
    let first = d.path().join("a");
    let mut p = vec![prepare(first.clone()), prepare(d.path().join("b"))];
    let result = commit_prepared(&mut p, |i, path, temp| {
        if i == 1 { atomic::write(&first, b"external")?; Err(AppError::new("FILE_PERMISSION", "test")) } else { atomic::commit(path, temp) }
    });
    assert_eq!(result.unwrap_err().code, "SWITCH_PARTIAL");
    assert_eq!(std::fs::read(first).unwrap(), b"external");
}
