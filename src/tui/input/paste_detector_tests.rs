use super::*;

#[test]
fn test_classify_paste_inline_small() {
    let detector = PasteDetector::new();
    let small_content = "a".repeat(200);

    let result = detector.classify_paste(&small_content);
    assert_eq!(result, PasteClassification::Inline);
}

#[test]
fn test_classify_paste_inline_exactly_200_chars() {
    let detector = PasteDetector::new();
    let exact_content = "a".repeat(200);

    let result = detector.classify_paste(&exact_content);
    assert_eq!(result, PasteClassification::Inline);
}

#[test]
fn test_classify_paste_attachment_over_threshold() {
    let detector = PasteDetector::new();
    let large_content = "a".repeat(201);

    let result = detector.classify_paste(&large_content);
    assert_eq!(result, PasteClassification::Attachment);
}

#[test]
fn test_classify_paste_attachment_large() {
    let detector = PasteDetector::new();
    let large_content = "a".repeat(10_000);

    let result = detector.classify_paste(&large_content);
    assert_eq!(result, PasteClassification::Attachment);
}

#[test]
fn test_classify_paste_rejected_exceeds_5mb() {
    let detector = PasteDetector::new();
    let huge_content = "a".repeat(5_000_001);

    let result = detector.classify_paste(&huge_content);
    match result {
        PasteClassification::Rejected(msg) => {
            assert!(msg.contains("exceeds"));
            assert!(msg.contains("5MB"));
        }
        _ => panic!("Expected Rejected, got {:?}", result),
    }
}

#[test]
fn test_classify_paste_unicode() {
    let detector = PasteDetector::new();
    let unicode_content = "🦀".repeat(100);

    assert_eq!(unicode_content.chars().count(), 100);
    let result = detector.classify_paste(&unicode_content);
    assert_eq!(result, PasteClassification::Inline);

    let unicode_large = "🦀".repeat(201);
    let result = detector.classify_paste(&unicode_large);
    assert_eq!(result, PasteClassification::Attachment);
}

#[test]
fn test_classify_paste_multiline() {
    let detector = PasteDetector::new();
    let multiline_small = "line1\nline2\nline3\n";
    let result = detector.classify_paste(multiline_small);
    assert_eq!(result, PasteClassification::Inline);

    let multiline_large = "a".repeat(50) + "\n" + &"b".repeat(160);
    let result = detector.classify_paste(&multiline_large);
    assert_eq!(result, PasteClassification::Attachment);
}

#[test]
fn test_custom_threshold() {
    let detector = PasteDetector::with_threshold(100, 5_000_000);
    let content = "a".repeat(101);

    let result = detector.classify_paste(&content);
    assert_eq!(result, PasteClassification::Attachment);

    let small_content = "a".repeat(100);
    let result = detector.classify_paste(&small_content);
    assert_eq!(result, PasteClassification::Inline);
}
