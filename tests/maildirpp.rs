use std::{collections::HashSet, fs};

use maildirs::{Maildir, Maildirs, MaildirsEntry};
use tempfile::tempdir;

#[test]
fn create() {
    let mdirs = Maildirs::new(tempdir().unwrap().path()).with_maildirpp(true);

    let subdir = mdirs.create("Subdir").unwrap();
    assert_eq!(subdir.exists(), true);
    assert_eq!(subdir.path(), mdirs.path().join(".Subdir"));

    let subdir = mdirs.create("Subdir/Subdir").unwrap();
    assert_eq!(subdir.exists(), true);
    assert_eq!(subdir.path(), mdirs.path().join(".Subdir").join(".Subdir"));

    let subdir = mdirs.create("Subdir/.Subdir").unwrap();
    assert_eq!(subdir.exists(), true);
    assert_eq!(subdir.path(), mdirs.path().join(".Subdir").join(".Subdir"));
}

#[test]
fn get() {
    let mdirs = Maildirs::new(tempdir().unwrap().path()).with_maildirpp(true);
    mdirs.create("Subdir/Subdir").unwrap();

    let subdir = mdirs.get("Subdir/Subdir").unwrap();
    assert_eq!(subdir.exists(), true);
    assert_eq!(subdir.path(), mdirs.path().join(".Subdir").join(".Subdir"));

    let subdir = mdirs.get(".Subdir/..Subdir").unwrap();
    assert_eq!(subdir.exists(), true);
    assert_eq!(subdir.path(), mdirs.path().join(".Subdir").join(".Subdir"));
}

#[test]
fn iter() {
    let mdirs = Maildirs::new(tempdir().unwrap().path()).with_maildirpp(true);
    mdirs.create("Subdir").unwrap();
    mdirs.create("Subdir/Subdir").unwrap();
    mdirs.create("A/.B/..C").unwrap();
    fs::create_dir(mdirs.path().join(".dot-no-maildir")).unwrap();
    fs::create_dir(mdirs.path().join("no-dot-no-maildir")).unwrap();

    // it should not list missing inbox
    let expected_mdirs = HashSet::from_iter([
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir")),
            name: "Subdir".into(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir/.Subdir")),
            name: "Subdir/Subdir".into(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".A").join(".B").join(".C")),
            name: "A/B/C".into(),
        },
    ]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);

    // create the inbox, then check that it is listed properly
    Maildir::from(mdirs.path()).create_all().unwrap();

    let expected_mdirs = HashSet::from_iter([
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path()),
            name: mdirs
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir")),
            name: "Subdir".into(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir/.Subdir")),
            name: "Subdir/Subdir".into(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".A").join(".B").join(".C")),
            name: "A/B/C".into(),
        },
    ]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);
}

#[test]
fn remove() {
    let mdirs = Maildirs::new(tempdir().unwrap().path()).with_maildirpp(true);
    mdirs.create("Subdir").unwrap();
    mdirs.create("Subdir/Subdir").unwrap();

    let expected_mdirs = HashSet::from_iter([
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir")),
            name: "Subdir".into(),
        },
        MaildirsEntry {
            maildirpp: true,
            maildir: Maildir::from(mdirs.path().join(".Subdir/.Subdir")),
            name: "Subdir/Subdir".into(),
        },
    ]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);

    mdirs.remove("Subdir/.Subdir").unwrap();

    let expected_mdirs = HashSet::from_iter([MaildirsEntry {
        maildirpp: true,
        maildir: Maildir::from(mdirs.path().join(".Subdir")),
        name: "Subdir".into(),
    }]);

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), expected_mdirs);

    mdirs.remove("..Subdir").unwrap();

    assert_eq!(mdirs.iter().collect::<HashSet<_>>(), HashSet::default());
}
