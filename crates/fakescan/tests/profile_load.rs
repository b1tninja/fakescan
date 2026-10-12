use std::path::PathBuf;

use fakescan::profile::Profile;

#[test]
fn loads_brother_wsd() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let p = Profile::load(&dir, "brother-wsd").expect("load");
    assert_eq!(p.connection, "wsd");
    assert!(p.optical.is_none());
    assert!(p.formats.iter().any(|f| f == "jpeg"));
}

#[test]
fn loads_brother_usb() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let p = Profile::load(&dir, "brother-usb").expect("load");
    assert_eq!(p.connection, "usb");
    assert_eq!(p.optical.as_ref().map(|o| o.x_dpi), Some(1200));
    assert!(p.feeder.sticky_ready);
}
