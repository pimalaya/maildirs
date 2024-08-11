use maildirs::Maildir;
use tempfile::tempdir;

#[test]
fn create() {
    let mdir = Maildir::from(tempdir().unwrap().into_path());
    assert_eq!(mdir.exists(), false);

    mdir.create_all().unwrap();
    assert_eq!(mdir.exists(), true);
    assert_eq!(mdir.create().is_err(), true);
    assert_eq!(mdir.create_all().is_ok(), true);
}

#[test]
fn remove() {
    let mdir = Maildir::from(tempdir().unwrap().into_path());

    mdir.create_all().unwrap();
    assert_eq!(mdir.exists(), true);

    mdir.remove_all().unwrap();
    assert_eq!(mdir.exists(), false);
}
