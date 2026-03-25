use std::path::PathBuf;

use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use tempfile::TempDir;

fn write_test_pdf(path: &PathBuf, page_count: usize) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_refs = Vec::new();

    for _ in 0..page_count {
        let content = Stream::new(Dictionary::new(), b"BT ET".to_vec());
        let content_id = doc.add_object(content);
        let page = dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "Parent" => Object::Reference(pages_id),
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
            "Contents" => Object::Reference(content_id),
        };
        page_refs.push(Object::Reference(doc.add_object(page)));
    }

    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(page_refs),
        "Count" => Object::Integer(page_count as i64),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog = dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    };
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.save(path).expect("failed to write test PDF");
}

#[test]
fn integration_merge_two_pdfs() {
    let dir = TempDir::new().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    write_test_pdf(&a, 2);
    write_test_pdf(&b, 3);

    let doc_a = Document::load(&a).unwrap();
    let doc_b = Document::load(&b).unwrap();
    let merged = super::merge::merge_documents(vec![doc_a, doc_b]).unwrap();

    assert_eq!(merged.get_pages().len(), 5);
}

#[test]
fn integration_split_every_two_pages() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 5);

    let doc = Document::load(&input).unwrap();
    let total = doc.get_pages().len();
    let all_pages: Vec<usize> = (1..=total).collect();
    let chunks = super::split::chunk_by_n(&all_pages, 2);

    assert_eq!(chunks, vec![vec![1, 2], vec![3, 4], vec![5]]);
}

#[test]
fn integration_compress_strips_page_thumbnails() {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let thumb_id = doc.add_object(Stream::new(Dictionary::new(), vec![1, 2, 3]));
    let page = dictionary! {
        "Type" => Object::Name(b"Page".to_vec()),
        "Parent" => Object::Reference(pages_id),
        "MediaBox" => Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
        "Thumb" => Object::Reference(thumb_id),
    };
    let page_id = doc.add_object(page);
    let pages = dictionary! {
        "Type" => Object::Name(b"Pages".to_vec()),
        "Kids" => Object::Array(vec![Object::Reference(page_id)]),
        "Count" => Object::Integer(1),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog = dictionary! {
        "Type" => Object::Name(b"Catalog".to_vec()),
        "Pages" => Object::Reference(pages_id),
    };
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", Object::Reference(catalog_id));

    super::compress::strip_thumbnails_direct(&mut doc);

    let page = doc.get_dictionary(page_id).unwrap();
    assert!(page.get(b"Thumb").is_err());
}

#[test]
fn integration_rotate_does_not_change_page_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 3);

    let mut doc = Document::load(&input).unwrap();
    let pages = doc.get_pages();
    for page_id in pages.values() {
        super::rotate::rotate_page_direct(&mut doc, *page_id, 90).unwrap();
    }

    assert_eq!(doc.get_pages().len(), 3);
}

#[test]
fn integration_reorder_roundtrip() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 4);

    let mut doc = Document::load(&input).unwrap();
    super::reorder::apply_reorder_direct(&mut doc, &[4, 3, 2, 1]).unwrap();

    assert_eq!(doc.get_pages().len(), 4);
}

#[test]
fn integration_remove_pages_reduces_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.pdf");
    write_test_pdf(&input, 5);

    let mut doc = Document::load(&input).unwrap();
    doc.delete_pages(&[2u32, 4u32]);

    assert_eq!(doc.get_pages().len(), 3);
}
