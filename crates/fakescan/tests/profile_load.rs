use std::path::PathBuf;

use fakescan::profile::Profile;

#[test]
fn loads_brother_wsd() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let p = Profile::load(&dir, "brother-wsd").expect("load");
    assert_eq!(p.connection, "wsd");
    assert!(p.optical.is_none());
    assert!(p.formats.iter().any(|f| f == "jfif"));
    assert!(p.formats.iter().any(|f| f == "tiff-single-uncompressed"));
    assert!(
        !p.formats.iter().any(|f| f == "dib"),
        "brother-wsd is Brother-shaped JFIF + TIFF; dib lives on the USB and lossless profiles"
    );
    assert!(p.jpeg_com);
    assert!(p.refuse_unknown);
}

#[test]
fn loads_brother_usb() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let p = Profile::load(&dir, "brother-usb").expect("load");
    assert_eq!(p.connection, "usb");
    assert_eq!(p.optical.as_ref().map(|o| o.x_dpi), Some(1200));
    assert!(p.feeder.sticky_ready);
    assert!(p.refuse_unknown);
}

#[test]
fn loads_wia_rich() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    let p = Profile::load(&dir, "wia-rich").expect("load");
    assert_eq!(p.connection, "wsd");
    assert!(p.formats.iter().any(|f| f == "pdf"));
    assert!(p.formats.iter().any(|f| f == "exif"));
}
