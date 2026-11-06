//! The `tf` member of a job.

use super::{TfBuffer, parse_jobs};

fn tf(json: &str) -> Result<Option<super::TransformFeedback>, String> {
    let text = format!(r#"[{{"key": "k", "stage": "vs"{json}}}]"#);
    parse_jobs(&text).map(|mut j| j.remove(0).tf)
}

#[test]
fn a_layout_has_its_buffers_in_order() {
    let t = tf(r#", "tf": [{"stride": 24, "outputs": [[1, 4], [5, 2]]}, [], null, {"stride": 4, "outputs": [[0, 1]]}]"#)
        .expect("parse")
        .expect("layout");
    assert_eq!(
        t.buffers[0],
        Some(TfBuffer {
            stride: 24,
            outputs: vec![(1, 4), (5, 2)]
        })
    );
    assert_eq!(t.buffers[1], None);
    assert_eq!(t.buffers[2], None);
    assert_eq!(t.strides(), [24, 0, 0, 4]);
}

#[test]
fn a_job_without_a_buffer_has_no_layout() {
    assert_eq!(tf(""), Ok(None));
    assert_eq!(tf(r#", "tf": null"#), Ok(None));
    assert_eq!(tf(r#", "tf": []"#), Ok(None));
    assert_eq!(tf(r#", "tf": [null, [], null, null]"#), Ok(None));
}

#[test]
fn a_malformed_layout_names_the_buffer() {
    let bad = |json: &str, want: &str| {
        let e = tf(json).expect_err(json);
        assert!(e.starts_with("job k: "), "{e}");
        assert!(e.contains(want), "{e}");
    };
    bad(r#", "tf": 3"#, "at most four");
    bad(r#", "tf": [null, null, null, null, null]"#, "at most four");
    bad(r#", "tf": [null, {"outputs": []}]"#, "buffer 1: no stride");
    bad(r#", "tf": [{"stride": 4}]"#, "buffer 0: no outputs");
    bad(
        r#", "tf": [{"stride": 16, "outputs": [[1, 5]]}]"#,
        "5 components",
    );
    bad(
        r#", "tf": [{"stride": 16, "outputs": [[32, 4]]}]"#,
        "location 32",
    );
    bad(
        r#", "tf": [{"stride": 12, "outputs": [[1, 4]]}]"#,
        "stride 12",
    );
    bad(r#", "tf": [{"stride": 0, "outputs": []}]"#, "stride 0");
    bad(r#", "tf": [{"stride": 4, "outputs": [1]}]"#, "pair");
}
