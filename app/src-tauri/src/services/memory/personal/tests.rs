use super::{
    document::{
        format_indexed_document, parse_content_elements, validate_document_structure,
        validate_no_heading_loss,
    },
    patch::{apply_patch_operations, extract_json_payload, MemoryPatchOperation},
};

const SAMPLE_DOC: &str = r#"# Personal Memory

## Personal Information
- User lives in Chicago.
- User speaks English and Hindi.

## Technical Projects
- Building a voice orchestrator in Rust.
- Works with Turso embedded database.
"#;

#[test]
fn test_parse_and_format_content_elements() {
    let elements = parse_content_elements(SAMPLE_DOC);
    assert_eq!(elements.len(), 7);
    assert_eq!(elements[0].raw_text, "# Personal Memory");
    assert_eq!(elements[1].raw_text, "## Personal Information");
    assert_eq!(elements[2].raw_text, "- User lives in Chicago.");
    assert_eq!(elements[3].raw_text, "- User speaks English and Hindi.");
    assert_eq!(elements[4].raw_text, "## Technical Projects");
    assert_eq!(
        elements[5].raw_text,
        "- Building a voice orchestrator in Rust."
    );
    assert_eq!(
        elements[6].raw_text,
        "- Works with Turso embedded database."
    );

    let formatted = format_indexed_document(&elements);
    // The `(kind)` label is what stops the model inferring element kind from a `#` prefix and
    // aiming a `replace` one index too low, which erased a section in production.
    assert!(formatted.contains("[1] (heading) # Personal Memory"));
    assert!(formatted.contains("[3] (bullet) - User lives in Chicago."));
    assert!(formatted.contains("[7] (bullet) - Works with Turso embedded database."));
}

#[test]
fn test_format_indexed_document_labels_every_kind() {
    let doc = "## Work\n- Job A\n\nA trailing paragraph.\n";
    let formatted = format_indexed_document(&parse_content_elements(doc));
    assert!(formatted.contains("[1] (heading) ## Work"));
    assert!(formatted.contains("[2] (bullet) - Job A"));
    assert!(formatted.contains("[3] (paragraph) A trailing paragraph."));
}

#[test]
fn test_validate_no_heading_loss_accepts_rename_and_growth() {
    let before = "## Work\n- Job A\n\n## Home\n- Job B\n";
    // Renaming a section changes its title but not its count.
    assert!(validate_no_heading_loss(before, "## Career\n- Job A\n\n## Home\n- Job B\n").is_ok());
    // Adding a section is fine.
    assert!(validate_no_heading_loss(
        before,
        "## Work\n- Job A\n\n## Home\n- Job B\n\n## Pets\n- Dog\n"
    )
    .is_ok());
}

#[test]
fn test_validate_no_heading_loss_rejects_erased_section() {
    let before = "## Work\n- Job A\n\n## Home\n- Job B\n";
    // The observed production failure: a `replace` aimed one index low overwrites a heading
    // with a bullet. The result is still well-formed markdown, so only the count catches it.
    let after = "## Work\n- Job A\n- Job B\n";
    assert!(validate_no_heading_loss(before, after).is_err());
}

#[test]
fn test_chained_section_anchor_lands_under_its_heading() {
    // The prompt's documented section-opening pattern at the end of a document: the bullet's
    // anchor is one past the last element, so the engine clamps it to an append. Descending
    // application must still produce heading-then-bullet.
    let base = "## Work\n- Job A\n\n## Home\n- Job B\n";
    let ops = vec![
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 5,
            text: "## Location".to_string(),
        },
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 6,
            text: "- Lives in Chicago.".to_string(),
        },
    ];
    let result = apply_patch_operations(base, &ops)
        .expect("Patch failed")
        .document;
    let lines: Vec<&str> = result.lines().map(str::trim).collect();
    let heading = lines.iter().position(|line| *line == "## Location");
    let bullet = lines.iter().position(|line| *line == "- Lives in Chicago.");
    assert!(heading.is_some(), "new section heading must be inserted");
    assert!(bullet.is_some(), "new section bullet must be inserted");
    assert!(
        bullet.unwrap() > heading.unwrap(),
        "bullet must land after its heading, got {lines:?}"
    );
}

#[test]
fn test_patch_insert_after_existing_bullet() {
    let ops = vec![MemoryPatchOperation {
        op: "insert_after".to_string(),
        index: 4,
        text: "- User enjoys playing badminton.".to_string(),
    }];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch failed")
        .document;
    assert!(result.contains("- User enjoys playing badminton."));
    assert!(result.contains("- User lives in Chicago."));
    assert!(result.contains("- Building a voice orchestrator in Rust."));
}

#[test]
fn test_patch_insert_after_prepend_zero() {
    let ops = vec![MemoryPatchOperation {
        op: "insert_after".to_string(),
        index: 0,
        text: "<!-- Profile Top -->".to_string(),
    }];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch failed")
        .document;
    assert!(result.starts_with("<!-- Profile Top -->"));
}

#[test]
fn test_patch_replace() {
    let ops = vec![MemoryPatchOperation {
        op: "replace".to_string(),
        index: 3,
        text: "- User lives in Austin.".to_string(),
    }];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch failed")
        .document;
    assert!(result.contains("- User lives in Austin."));
    assert!(!result.contains("- User lives in Chicago."));
    // Anchors preserved
    assert!(result.contains("- User speaks English and Hindi."));
}

#[test]
fn test_patch_delete() {
    let ops = vec![MemoryPatchOperation {
        op: "delete".to_string(),
        index: 7,
        text: String::new(),
    }];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch failed")
        .document;
    assert!(!result.contains("Works with Turso embedded database."));
    assert!(result.contains("- Building a voice orchestrator in Rust."));
}

#[test]
fn test_patch_out_of_bounds_resilience() {
    let ops = vec![
        MemoryPatchOperation {
            op: "replace".to_string(),
            index: 99,
            text: "- Phantom fact.".to_string(),
        },
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 3,
            text: "- Added valid fact.".to_string(),
        },
    ];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch should not error")
        .document;
    assert!(result.contains("- Added valid fact."));
    assert!(!result.contains("- Phantom fact."));
}

#[test]
fn test_patch_descending_sort_prevents_index_drift() {
    let ops = vec![
        MemoryPatchOperation {
            op: "delete".to_string(),
            index: 7,
            text: String::new(),
        },
        MemoryPatchOperation {
            op: "replace".to_string(),
            index: 3,
            text: "- User lives in Austin.".to_string(),
        },
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 4,
            text: "- User enjoys hiking.".to_string(),
        },
    ];

    let result = apply_patch_operations(SAMPLE_DOC, &ops)
        .expect("Patch failed")
        .document;
    assert!(result.contains("- User lives in Austin."));
    assert!(!result.contains("- User lives in Chicago."));
    assert!(result.contains("- User enjoys hiking."));
    assert!(!result.contains("Works with Turso embedded database."));
}

#[test]
fn test_validate_document_structure_success() {
    assert!(validate_document_structure(SAMPLE_DOC).is_ok());
}

#[test]
fn test_validate_document_structure_nameless_heading_fails() {
    let doc = "## \n- Bullet one\n";
    assert!(validate_document_structure(doc).is_err());
}

#[test]
fn test_validate_document_structure_duplicate_heading_fails() {
    let doc = "## Work\n- Job A\n\n## Work\n- Job B\n";
    assert!(validate_document_structure(doc).is_err());
}

#[test]
fn test_validate_document_structure_no_headings_fails() {
    let doc = "- Just a bullet\n- Another bullet\n";
    assert!(validate_document_structure(doc).is_err());
}

#[test]
fn test_extract_json_payload_with_fences() {
    let fenced = "```json\n{\"edits\": []}\n```";
    assert_eq!(extract_json_payload(fenced), "{\"edits\": []}");

    let raw = "{\"edits\": []}";
    assert_eq!(extract_json_payload(raw), "{\"edits\": []}");
}

#[test]
fn test_heading_destroying_replace_is_refused_but_batch_otherwise_applies() {
    // The Sprint 2 stall: refusing the whole batch left every suggestion pending forever and the
    // document stopped changing. Only the offending operation may be dropped.
    let base = "## Work\n- Job A\n\n## Home\n- Job B\n";
    let ops = vec![
        MemoryPatchOperation {
            op: "replace".to_string(),
            index: 3,
            // Targets the `## Home` heading with bullet text.
            text: "- Job B and commuting.".to_string(),
        },
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 1,
            text: "- Job A, remotely.".to_string(),
        },
    ];
    let report = apply_patch_operations(base, &ops).expect("Patch failed");
    assert_eq!(report.rejected.len(), 1, "exactly the heading-erasing op");
    assert_eq!(report.rejected[0].position, 0);
    assert!(
        report.rejected[0].reason.contains("erase the section"),
        "reason must name the failure: {}",
        report.rejected[0].reason
    );
    // The safe operation in the same batch still landed.
    assert!(report.document.contains("- Job A, remotely."));
    // And the section it would have destroyed is intact.
    assert!(report.document.contains("## Home"));
    assert!(!report.document.contains("- Job B and commuting."));
}

#[test]
fn test_heading_to_heading_replace_is_allowed() {
    // Renaming a section is legitimate and must not be refused.
    let base = "## Work\n- Job A\n\n## Home\n- Job B\n";
    let ops = vec![MemoryPatchOperation {
        op: "replace".to_string(),
        index: 3,
        text: "## Household".to_string(),
    }];
    let report = apply_patch_operations(base, &ops).expect("Patch failed");
    assert!(report.rejected.is_empty(), "a rename must not be refused");
    assert!(report.document.contains("## Household"));
    assert!(!report.document.contains("## Home"));
}

#[test]
fn test_rejected_positions_survive_descending_reorder() {
    // `position` must index the caller's original slice, not the engine's sorted order, or the
    // accept path cannot map a rejection back to its suggestion row.
    let base = "## Work\n- Job A\n\n## Home\n- Job B\n";
    let ops = vec![
        MemoryPatchOperation {
            op: "replace".to_string(),
            index: 3,
            text: "- erased heading".to_string(),
        },
        MemoryPatchOperation {
            op: "insert_after".to_string(),
            index: 1,
            text: "- safe".to_string(),
        },
        MemoryPatchOperation {
            op: "replace".to_string(),
            index: 2,
            text: "- Job A2".to_string(),
        },
    ];
    let report = apply_patch_operations(base, &ops).expect("Patch failed");
    assert_eq!(report.rejected.len(), 1);
    assert_eq!(
        report.rejected[0].position, 0,
        "rejection must carry the caller's original index"
    );
    assert_eq!(report.rejected[0].operation.index, 3);
}
