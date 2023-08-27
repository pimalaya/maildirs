use std::fs;
#[cfg(unix)]
use std::{borrow::Cow, ffi::OsStr, os::unix::ffi::OsStrExt};
#[cfg(windows)]
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
};

use mail_parser::Message;
use maildirpp::{Error, Flag, Maildir};
use percent_encoding::percent_decode;
use tempfile::tempdir;
use walkdir::WalkDir;

const TESTDATA_DIR: &str = "tests/testdata";
const MAILDIR: &str = "maildir";
const SUBMAILDIRS: &str = "submaildirs";

// `cargo package` doesn't package files with certain characters, such as
// colons, in the name, so we percent-decode the file names when copying the
// data for the tests.
// This code can likely be improved (for correctness, particularly on Windows)
// but there's no good docs on what `cargo package` does percent-encoding for,
// and how it deals with multibyte characters in filenames. In practice this
// code works fine for this crate, because we have a restricted set of ASCII
// characters in the filenames.
fn with_maildir<F>(name: &str, func: F)
where
    F: FnOnce(Maildir),
{
    let tmp_dir = tempdir().expect("could not create temporary directory");
    let tmp_path = tmp_dir.path();
    for entry in WalkDir::new(TESTDATA_DIR) {
        let entry = entry.expect("directory walk error");
        let relative = entry.path().strip_prefix(TESTDATA_DIR).unwrap();
        if relative.parent().is_none() {
            continue;
        }

        #[cfg(unix)]
        let decoded_bytes: Cow<[u8]> = percent_decode(relative.as_os_str().as_bytes()).into();
        #[cfg(unix)]
        let decoded = OsStr::from_bytes(&decoded_bytes);

        #[cfg(windows)]
        let decoded_bytes = relative
            .as_os_str()
            .encode_wide()
            .map(|b| b as u8)
            .collect::<Vec<_>>();
        #[cfg(windows)]
        let decoded_bytes = percent_decode(decoded_bytes.as_slice())
            .map(|b| (if b == b':' { b';' } else { b }) as u16)
            .collect::<Vec<_>>();
        #[cfg(windows)]
        let decoded = OsString::from_wide(decoded_bytes.as_slice());

        if entry.path().is_dir() {
            fs::create_dir(tmp_path.join(&decoded)).expect("could not create directory");
        } else {
            fs::copy(entry.path(), tmp_path.join(decoded)).expect("could not copy test data");
        }
    }
    func(Maildir::from(tmp_path.join(name)));
}

fn with_maildir_empty<F>(name: &str, func: F)
where
    F: FnOnce(Maildir),
{
    let tmp_dir = tempdir().expect("could not create temporary directory");
    let tmp_path = tmp_dir.path();
    func(Maildir::from(tmp_path.join(name)));
}

#[test]
fn maildir_count() {
    with_maildir(MAILDIR, |maildir| {
        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.count_new(), 1);
    });
}

#[test]
fn maildir_list() {
    with_maildir(MAILDIR, |maildir| {
        let mut iter = maildir.list_new();
        let entry1 = iter.next().unwrap().unwrap();
        let bytes = entry1.to_bytes().unwrap();
        let msg1 = Message::parse(&bytes).unwrap();
        assert_eq!(entry1.id(), "1463941010.5f7fa6dd4922c183dc457d033deee9d7");
        assert_eq!(msg1.subject(), Some("test"));
        assert_eq!(entry1.has_flag(Flag::Seen), false);
        let second_entry = iter.next();
        assert!(second_entry.is_none());

        let mut iter = maildir.list_cur();
        let entry1 = iter.next().unwrap().unwrap();
        let bytes = entry1.to_bytes().unwrap();
        let msg1 = Message::parse(&bytes).unwrap();
        assert_eq!(entry1.id(), "1463868505.38518452d49213cb409aa1db32f53184");
        assert_eq!(msg1.subject(), Some("test"));
        assert_eq!(entry1.has_flag(Flag::Seen), true);
        let entry2 = iter.next();
        assert!(entry2.is_none());
    })
}

#[test]
fn maildir_list_subdirs() {
    with_maildir(SUBMAILDIRS, |maildir| {
        let subdirs: Vec<_> = maildir
            .folders()
            .inspect(|d| println!("{:?}", d))
            .map(|dir| {
                dir.unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert_eq!(2, subdirs.len());
        assert!(subdirs.contains(&".Subdir1".into()));
        assert!(subdirs.contains(&".Subdir2".into()));
        assert!(!subdirs.contains(&"..Subdir3".into()));
    });
}

#[test]
fn maildir_find() {
    with_maildir(MAILDIR, |maildir| {
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_some(),
            true
        );
        assert_eq!(
            maildir
                .find("1463868505.38518452d49213cb409aa1db32f53184")
                .is_some(),
            true
        );
    })
}

#[test]
fn check_delete() {
    with_maildir(MAILDIR, |maildir| {
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_some(),
            true
        );
        assert_eq!(
            maildir
                .delete("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_ok(),
            true
        );
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .is_some(),
            false
        );
    })
}

#[test]
fn check_copy_and_move() {
    with_maildir(MAILDIR, |maildir| {
        with_maildir(SUBMAILDIRS, |submaildir| {
            let id = "1463868505.38518452d49213cb409aa1db32f53184";

            // check that we cannot copy a message from and to the same maildir
            assert!(matches!(
                maildir.copy_to(id, &maildir).unwrap_err(),
                Error::CopyEmailSamePathError(_),
            ));

            // check that the message is present in "maildir" but not in "submaildir"
            assert!(maildir.find(id).is_some());
            assert!(submaildir.find(id).is_none());
            // also check that the failed self-copy a few lines up didn't corrupt the
            // message file.
            let body = maildir.find(id).unwrap().to_bytes().unwrap();
            let msg = Message::parse(&body).unwrap();
            assert!(msg.date().is_some());

            // copy the message from "maildir" to "submaildir"
            maildir.copy_to(id, &submaildir).unwrap();

            // check that the message is now present in both
            assert!(maildir.find(id).is_some());
            assert!(submaildir.find(id).is_some());

            // move the message from "submaildir" to "maildir"
            submaildir.move_to(id, &maildir).unwrap();

            // check that the message is now only present in "maildir"
            assert!(maildir.find(id).is_some());
            assert!(submaildir.find(id).is_none());
        })
    })
}

#[test]
fn mark_read() {
    with_maildir(MAILDIR, |maildir| {
        assert_eq!(
            maildir
                .find("1463941010.5f7fa6dd4922c183dc457d033deee9d7")
                .unwrap()
                .move_to_cur()
                .unwrap(),
            ()
        );
    });
}

const TEST_MAIL_BODY: &[u8] = b"Return-Path: <of82ecuq@cip.cs.fau.de>
X-Original-To: of82ecuq@cip.cs.fau.de
Delivered-To: of82ecuq@cip.cs.fau.de
Received: from faui0fl.informatik.uni-erlangen.de (unknown [IPv6:2001:638:a000:4160:131:188:60:117])
        by faui03.informatik.uni-erlangen.de (Postfix) with ESMTP id 466C1240A3D
        for <of82ecuq@cip.cs.fau.de>; Fri, 12 May 2017 10:09:45 +0000 (UTC)
Received: by faui0fl.informatik.uni-erlangen.de (Postfix, from userid 303135)
        id 389CC10E1A32; Fri, 12 May 2017 12:09:45 +0200 (CEST)
To: of82ecuq@cip.cs.fau.de
MIME-Version: 1.0
Content-Type: text/plain; charset=\"UTF-8\"
Content-Transfer-Encoding: 8bit
Message-Id: <20170512100945.389CC10E1A32@faui0fl.informatik.uni-erlangen.de>
Date: Fri, 12 May 2017 12:09:45 +0200 (CEST)
From: of82ecuq@cip.cs.fau.de (Johannes Schilling)
Subject: maildir delivery test mail

Today is Boomtime, the 59th day of Discord in the YOLD 3183";

#[test]
fn check_store_new() {
    with_maildir_empty("maildir2", |maildir| {
        assert_eq!(maildir.count_new(), 0);
        let entry = maildir.store_new(TEST_MAIL_BODY).unwrap();
        assert_eq!(maildir.count_new(), 1);

        let entry = maildir.find(entry.id()).unwrap();

        let msg = entry.to_bytes().unwrap();
        let msg = Message::parse(&msg).unwrap();
        assert_eq!(
            msg.body_text(0).unwrap(),
            "Today is Boomtime, the 59th day of Discord in the YOLD 3183"
        );
    });
}

#[test]
fn check_store_cur() {
    with_maildir_empty("maildir2", |maildir| {
        let testflags = "FRS";
        let want = vec![Flag::Flagged, Flag::Replied, Flag::Seen];

        assert_eq!(maildir.count_cur(), 0);
        let mut entry = maildir.store_cur(TEST_MAIL_BODY).unwrap();
        assert_eq!(maildir.count_cur(), 1);

        for flag in testflags.chars() {
            entry.set_flag(flag.try_into().unwrap()).unwrap();
        }

        let mut iter = maildir.list_cur();
        let first = iter.next().unwrap().unwrap();
        let mut got = first.flags().copied().collect::<Vec<Flag>>();
        got.sort_by(|f1, f2| f1.as_ref().cmp(f2.as_ref()));
        assert_eq!(got, want);
    });
}

#[test]
fn check_flag_fiddling() {
    with_maildir_empty("maildir2", |maildir| {
        let mut entry = maildir.store_cur(TEST_MAIL_BODY).unwrap();
        entry.set_flag(Flag::Seen).unwrap();
        entry.set_flag(Flag::Replied).unwrap();

        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.find(entry.id()).unwrap().flags_to_string(), "RS");
        entry.unset_flag(Flag::Seen).unwrap();
        assert_eq!(maildir.find(entry.id()).unwrap().flags_to_string(), "R");
        entry.set_flag(Flag::Replied).unwrap();
        entry.set_flag(Flag::Flagged).unwrap();
        assert_eq!(maildir.find(entry.id()).unwrap().flags_to_string(), "FR");
        entry.set_flag(Flag::Seen).unwrap();
        entry.set_flag(Flag::Flagged).unwrap();
        assert_eq!(maildir.find(entry.id()).unwrap().flags_to_string(), "FRS");
    });
}
