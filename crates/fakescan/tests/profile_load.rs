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

#[test]
fn loads_feeder_tray_profiles() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../profiles");
    for (id, cap) in [
        ("feeder-tray5", 5),
        ("feeder-tray50", 50),
        ("feeder-duplex30", 30),
        ("feeder-a4only", 20),
    ] {
        let p = Profile::load(&dir, id).expect(id);
        assert_eq!(p.feeder.capacity, Some(cap), "{id}");
        assert_eq!(p.connection, "wsd");
    }
    let b = Profile::load(&dir, "brother-wsd").unwrap();
    assert_eq!(b.feeder.capacity, None);
}
