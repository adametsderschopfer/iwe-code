use super::*;

#[test]
fn recognizes_hex_and_function_colors_at_utf8_byte_ranges() {
    let line = "цвет: #abc; #1234, #417690ff, rgb(65 118 144 / 50%), hsl(120deg, 100%, 50%)";
    let colors: Vec<_> = literals(line)
        .map(|(range, color)| (&line[range], color.0))
        .collect();
    assert_eq!(
        colors,
        [
            ("#abc", [170, 187, 204, 255]),
            ("#1234", [17, 34, 51, 68]),
            ("#417690ff", [65, 118, 144, 255]),
            ("rgb(65 118 144 / 50%)", [65, 118, 144, 128]),
            ("hsl(120deg, 100%, 50%)", [0, 255, 0, 255]),
        ],
    );
    assert_eq!(
        literals("#abcde #123456789 #fffffg word#abc ##fff myrgb(1, 2, 3) rgb(10, , 20, 30) rgb(1 2 3 0.5)").count(),
        0
    );
}

#[test]
fn preserves_short_values_and_alpha_when_possible() {
    assert_eq!(Rgba([204, 0, 17, 255]).css("#ABC"), "#C01");
    assert_eq!(Rgba([204, 0, 17, 128]).css("#ABC"), "#CC001180");
    assert_eq!(Rgba([204, 0, 17, 255]).css("#abcdef00"), "#cc0011ff");
}

#[test]
fn hsv_round_trips_the_original_color() {
    for rgba in [[65, 118, 144, 180], [128, 128, 128, 0], [255, 0, 255, 255]] {
        let color = Rgba(rgba);
        assert_eq!(Rgba::from_hsv(color.hsv(), rgba[3]), color);
    }
}
