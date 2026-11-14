use super::{Line, read_line};
use tokio::io::BufReader;

const LIMIT: usize = 8;

#[tokio::test]
async fn reads_complete_lines_one_by_one() {
    let mut input = BufReader::new(&b"one\ntwo\n"[..]);
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::Text("one".to_owned()))
    );
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::Text("two".to_owned()))
    );
    assert_eq!(read_line(&mut input, LIMIT).await.expect("readable"), None);
}

#[tokio::test]
async fn reads_an_empty_line_and_a_final_partial_line() {
    let mut input = BufReader::new(&b"\nlast"[..]);
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::Text(String::new()))
    );
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::Text("last".to_owned()))
    );
    assert_eq!(read_line(&mut input, LIMIT).await.expect("readable"), None);
}

#[tokio::test]
async fn discards_lines_above_the_cap_and_keeps_reading() {
    let long = "x".repeat(LIMIT + 1);
    let input = format!("{long}\nafter\n");
    let mut input = BufReader::new(input.as_bytes());
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::TooLong)
    );
    assert_eq!(
        read_line(&mut input, LIMIT).await.expect("readable"),
        Some(Line::Text("after".to_owned()))
    );
}
