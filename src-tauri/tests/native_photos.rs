// IA'GS: real native helper regression using generated, non-private image fixtures.
use std::{path::PathBuf, process::Command};
#[test]
#[ignore = "Requires compiled native helper and macOS graphics services; run npm run test:native"]
fn transparent_holes_orientation_and_brush_keep_pixels_consistent() {
    let helper =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../engines/macos/arm64/bin/iags-photo");
    assert!(helper.is_file());
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    let output = temp.path().join("prepared");
    std::fs::create_dir(&input).unwrap();
    let mut img = image::RgbaImage::from_pixel(80, 60, image::Rgba([0, 0, 0, 0]));
    for y in 8..52 {
        for x in 8..72 {
            img.put_pixel(x, y, image::Rgba([200, 40, 60, 255]));
        }
    }
    for y in 25..35 {
        for x in 35..45 {
            img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }
    img.put_pixel(4, 4, image::Rgba([30, 120, 210, 255])); // Detached genuine component stays.
    img.save(input.join("a.png")).unwrap();
    img.save(input.join("b.png")).unwrap();
    let run = Command::new(&helper)
        .args([
            "prepare",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
            "2400",
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result = image::open(output.join("output/frame_000001.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(result.dimensions(), (80, 60));
    assert_eq!(result.get_pixel(40, 30)[3], 0);
    assert_eq!(result.get_pixel(4, 4)[3], 255);
    assert_eq!(&result.get_pixel(20, 15).0[..3], &[200, 40, 60]);
    assert_eq!(
        std::fs::read(input.join("a.png")).unwrap(),
        std::fs::read(output.join("originals/frame_000001.png")).unwrap()
    );
    let edit = serde_json::json!({"id":"frame_000001","strokes":[{"erase":true,"radius":0.03,"points":[[0.25,0.25]]}],"select":null,"enabled":true,"reviewed":true});
    std::fs::write(output.join("edit.json"), serde_json::to_vec(&edit).unwrap()).unwrap();
    let run = Command::new(&helper)
        .args([
            "edit",
            output.to_str().unwrap(),
            output.join("edit.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let edited = image::open(output.join("output/frame_000001.png"))
        .unwrap()
        .to_rgba8();
    let preview = image::open(output.join("previews/frame_000001.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(edited.get_pixel(20, 15)[3], 0);
    for (a, b) in edited.pixels().zip(preview.pixels()) {
        assert_eq!(&a.0[..3], &b.0[..3]);
    }
}
