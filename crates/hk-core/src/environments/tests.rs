use super::*;
use std::fs;

#[test]
fn file_save_rejects_stale_revision_and_keeps_backup() {
    let t = tempfile::tempdir().unwrap();
    let file = t.path().join("config.toml");
    fs::write(&file, "model = 'a'\n").unwrap();
    let first = read_text(&file, &[t.path().to_path_buf()]).unwrap();
    fs::write(&file, "model = 'b'\n").unwrap();
    assert!(matches!(
        save_text(
            &file,
            &first.revision,
            "new",
            &[t.path().to_path_buf()],
            &t.path().join("history")
        ),
        Err(crate::HkError::Conflict(_))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "model = 'b'\n");
    let current = read_text(&file, &[t.path().to_path_buf()]).unwrap();
    let saved = save_text(
        &file,
        &current.revision,
        "new",
        &[t.path().to_path_buf()],
        &t.path().join("history"),
    )
    .unwrap();
    assert_eq!(saved.content, "new");
    assert_ne!(saved.revision, current.revision);
    assert_eq!(
        fs::read(t.path().join("history").join(&current.revision)).unwrap(),
        current.content.as_bytes()
    );
}

#[test]
fn text_read_rejects_outside_root_and_binary() {
    let t = tempfile::tempdir().unwrap();
    let file = t.path().join("file");
    fs::write(&file, [0, 255]).unwrap();
    assert!(read_text(&file, &[]).is_err());
    assert!(read_text(&file, &[t.path().to_path_buf()]).is_err());
}
