use std::collections::HashSet;

use maildirs::{Flag, Maildirs};
use tempfile::tempdir;

#[test]
fn write_maildir_entry() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path());
    let mdir = mdirs.create("mdir").unwrap();

    let entry = mdir.write_new(b"data").unwrap();
    let expected_path = Some(mdir.path().join("new"));
    assert_eq!(entry.path().parent(), expected_path.as_deref());
    assert!(entry.flags().unwrap().is_empty());

    let entry = mdir.write_cur(b"data", [Flag::Passed, Flag::Seen]).unwrap();
    let expected_path = Some(mdir.path().join("cur"));
    assert_eq!(entry.path().parent(), expected_path.as_deref());

    let expected_flags = HashSet::from_iter([Flag::Seen, Flag::Passed]);
    assert_eq!(entry.flags().unwrap(), expected_flags);
}

#[test]
fn manage_maildir_entries() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path());

    let a = mdirs.create("a").unwrap();
    let b = mdirs.create("b/c").unwrap();
    assert_eq!(a.read().unwrap().count(), 0);
    assert_eq!(b.read().unwrap().count(), 0);

    let entry = a.write_cur(b"data", None).unwrap();
    assert_eq!(a.read().unwrap().count(), 1);
    assert_eq!(b.read().unwrap().count(), 0);

    entry.copy(&b).unwrap();
    assert_eq!(a.read().unwrap().count(), 1);
    assert_eq!(b.read().unwrap().count(), 1);

    entry.r#move(&b).unwrap();
    assert_eq!(a.read().unwrap().count(), 0);
    assert_eq!(b.read().unwrap().count(), 1);
}

#[test]
fn change_maildir_entry_flags() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path());
    let mdir = mdirs.create("mdir").unwrap();
    let mut entry = mdir.write_cur(b"data", [Flag::Passed]).unwrap();
    let expected_flags = HashSet::from_iter([Flag::Passed]);
    assert_eq!(entry.flags().unwrap(), expected_flags);

    entry.insert_flag(Flag::Seen).unwrap();
    let expected_flags = HashSet::from_iter([Flag::Passed, Flag::Seen]);
    assert_eq!(entry.flags().unwrap(), expected_flags);

    entry.update_flags([Flag::Draft, Flag::Passed]).unwrap();
    let expected_flags = HashSet::from_iter([Flag::Passed, Flag::Draft]);
    assert_eq!(entry.flags().unwrap(), expected_flags);

    entry.remove_flag(Flag::Passed).unwrap();
    let expected_flags = HashSet::from_iter([Flag::Draft]);
    assert_eq!(entry.flags().unwrap(), expected_flags);
}
