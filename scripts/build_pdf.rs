fn main() {
    let mut doc = lopdf::Document::with_version("1.4");
    let page_id = doc.new_object_id();
    doc.set_object(page_id, lopdf::dictionary! {
        "Type" => "Page",
        "Parent" => 0u32,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    });
    let pages_id = doc.new_object_id();
    doc.set_object(pages_id, lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1u32,
    });
    let catalog_id = doc.new_object_id();
    doc.set_object(catalog_id, lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save("target/test.pdf").unwrap();
}
