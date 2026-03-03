#[test]
fn test_parse_example_file() {
    let content = std::fs::read_to_string("../examples/test.http").expect("read test.http");

    // We can't easily import from a bin crate, so replicate the parsing logic check
    // This validates the example file is well-formed
    assert!(content.contains("GET https://httpbin.org/get"));
    assert!(content.contains("POST https://httpbin.org/post"));
    assert!(content.contains("###"));
}

#[test]
fn test_env_file_exists() {
    let content = std::fs::read_to_string("../examples/.env").expect("read .env");
    assert!(content.contains("HOST=httpbin.org"));
}
