use std::{collections::HashSet, fs};

use maildirs::{Flag, Maildir, Maildirs};
use tempfile::tempdir;

#[test]
fn create_maildir() {
    let mdir = Maildir::from(tempdir().unwrap().into_path());
    assert_eq!(mdir.exists(), false);

    mdir.create_all().unwrap();
    assert_eq!(mdir.exists(), true);
    assert_eq!(mdir.create().is_err(), true);
    assert_eq!(mdir.create_all().is_ok(), true);
}

#[test]
fn remove_maildir() {
    let mdir = Maildir::from(tempdir().unwrap().into_path());

    mdir.create_all().unwrap();
    assert_eq!(mdir.exists(), true);

    mdir.remove_all().unwrap();
    assert_eq!(mdir.exists(), false);
}

#[test]
fn add_maildir_to_maildirs() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path());
    let a = mdirs.create("a").unwrap();
    assert_eq!(a.exists(), true);
    assert_eq!(mdirs.path().join("a"), a.path());
}

#[test]
fn add_maildirpp_to_maildirs() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path()).with_maildirpp(true);
    let a = mdirs.create("a").unwrap();
    assert_eq!(a.exists(), true);
    assert_eq!(mdirs.path().join(".a"), a.path());
}

#[test]
fn list_maildir_from_maildirs() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path());
    mdirs.create("a").unwrap();
    mdirs.create("b").unwrap();
    mdirs.create("c").unwrap();

    fs::create_dir(mdirs.path().join(".dot-no-maildir")).unwrap();
    fs::create_dir(mdirs.path().join("no-dot-no-maildir")).unwrap();
    Maildir::from(mdirs.path().join(".dot-maildir"))
        .create_all()
        .unwrap();
    Maildir::from(mdirs.path().join("no-dot-maildir"))
        .create_all()
        .unwrap();

    let expected_mdirs = HashSet::from_iter([
        Maildir::from(mdirs.path().join("a")),
        Maildir::from(mdirs.path().join("b")),
        Maildir::from(mdirs.path().join("c")),
        Maildir::from(mdirs.path().join("no-dot-maildir")),
    ]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);
}

#[test]
fn list_maildirpp_from_maildirs() {
    let mdirs = Maildirs::new(tempdir().unwrap().into_path()).with_maildirpp(true);
    mdirs.create("a").unwrap();
    mdirs.create("b").unwrap();
    mdirs.create("c").unwrap();

    fs::create_dir(mdirs.path().join(".dot-no-maildir")).unwrap();
    fs::create_dir(mdirs.path().join("no-dot-no-maildir")).unwrap();
    Maildir::from(mdirs.path().join(".dot-maildir"))
        .create_all()
        .unwrap();
    Maildir::from(mdirs.path().join("no-dot-maildir"))
        .create_all()
        .unwrap();

    let expected_mdirs = HashSet::from_iter([
        Maildir::from(mdirs.path()),
        Maildir::from(mdirs.path().join(".a")),
        Maildir::from(mdirs.path().join(".b")),
        Maildir::from(mdirs.path().join(".c")),
        Maildir::from(mdirs.path().join(".dot-maildir")),
    ]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);
}

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
    let b = mdirs.create("b").unwrap();
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
