use super::HtmlFileUri;

#[test]
fn file_uri_escapes_spaces_unicode_and_windows_separators() {
    let uri = HtmlFileUri::from_path(std::path::Path::new(r"C:\Users\Hiroyuki Furuno\画像.png"));

    assert_eq!(
        uri,
        "file:///C:/Users/Hiroyuki%20Furuno/%E7%94%BB%E5%83%8F.png"
    );
}
